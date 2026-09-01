# M1 — Identity, Configuration, and the Audit Write Path

**Date:** 2026-09-01
**Status:** Approved
**Milestone:** M1 of the [architecture design](2026-08-31-grubsi-architecture-design.md)
**Depends on:** M0 (walking skeleton) — complete, 28 commits, 31 Rust + 6 Vitest tests

---

## 1. Scope

M1 makes the system know who is acting. It delivers:

- Users, roles, permissions, and boot-time permission seeding
- Two authentication paths: password for back-office, device-plus-PIN for the service floor
- Server-side sessions, with revocation
- First-run setup guarded by a console token
- Restaurant settings, including the service-day cutoff M4 depends on
- Real audit attribution on every mutation
- The admin shell UI and the terminal lock screen
- Write-pool privatisation, closing Ruling J from M0

**Done when** an admin logs in from a browser, creates staff users, enrols a floor tablet, and staff PIN in and out on it — with every mutation attributed to a real user and visible in the audit log.

### 1.1 A decision that supersedes the architecture spec

The architecture design's §7 specifies username + Argon2id password for all staff. **M1 supersedes that** with two paths, because a steward typing a password on a shared tablet mid-service is not a workable floor interaction:

| | Back office | Service floor |
|---|---|---|
| Who | Admin, Manager | Steward, Cashier, Kitchen, Bar |
| Device | Any browser, no enrolment | Tablet enrolled once |
| Credential | Username + password | Device token, then per-person PIN |
| Session | One | Two: device (long) + staff (short, auto-locks) |

Both paths mint the same `sessions` row and produce the same request context, so authorization and audit see one model. Only the proof of identity differs.

Architecture design §7 should be amended to match once M1 lands.

### 1.2 Scope decision recorded

M1 was proposed as a split — identity core now, terminals as a separate milestone before M4, on the grounds that M2 and M3 are browser-only admin work that needs no terminals. **The split was declined; M1 stays whole.** This puts M1 at roughly 22 tasks against M0's 13. Recorded here so the plan's size is not a surprise.

---

## 2. Identity model

### 2.1 A PIN cannot identify a person

If staff type only a PIN, the server must determine whose it is. PIN hashes are salted, so no lookup is possible — the alternatives are Argon2-verifying against every user in turn (roughly 20 staff × 50 ms, a second per attempt) or storing PINs weakly enough to index. Neither is acceptable.

**So the floor flow is: pick your name, then enter your PIN.** One verify against one user. PINs need not be unique. This is also what most POS terminals already do, so it costs nothing in familiarity.

### 2.2 Schema

All tables `STRICT`. All ids UUIDv7 as `BLOB`. All timestamps UTC via `infra::time::now_iso()`. `STRICT` has no boolean type, so flags are `INTEGER` holding 0 or 1.

```sql
users(
  id BLOB PRIMARY KEY NOT NULL,
  username TEXT NOT NULL UNIQUE,
  display_name TEXT NOT NULL,
  password_hash TEXT,                 -- NULL for PIN-only floor staff
  pin_hash TEXT,                      -- NULL for password-only back office
  role_id BLOB NOT NULL REFERENCES roles(id),
  active INTEGER NOT NULL DEFAULT 1,
  failed_pin_attempts INTEGER NOT NULL DEFAULT 0,   -- PIN lockout only; see §3.2
  locked_until TEXT,                                -- PIN lockout only; see §3.2
  created_at TEXT NOT NULL,
  updated_at TEXT NOT NULL
) STRICT;

roles(
  id BLOB PRIMARY KEY NOT NULL,
  name TEXT NOT NULL UNIQUE,
  is_system INTEGER NOT NULL DEFAULT 0,   -- seeded roles cannot be deleted
  created_at TEXT NOT NULL
) STRICT;

permissions(code TEXT PRIMARY KEY NOT NULL) STRICT;   -- seeded from core::Permission

role_permissions(
  role_id BLOB NOT NULL REFERENCES roles(id) ON DELETE CASCADE,
  permission_code TEXT NOT NULL REFERENCES permissions(code),
  PRIMARY KEY (role_id, permission_code)
) STRICT;

devices(
  id BLOB PRIMARY KEY NOT NULL,
  name TEXT NOT NULL UNIQUE,
  enrolled_at TEXT NOT NULL,
  last_seen_at TEXT,
  revoked_at TEXT,
  created_by BLOB REFERENCES users(id)
) STRICT;

device_enrolments(
  id BLOB PRIMARY KEY NOT NULL,
  code_hash TEXT NOT NULL UNIQUE,
  device_name TEXT NOT NULL,
  expires_at TEXT NOT NULL,
  consumed_at TEXT,
  consumed_device_id BLOB REFERENCES devices(id),
  created_by BLOB NOT NULL REFERENCES users(id),
  created_at TEXT NOT NULL
) STRICT;

sessions(
  id BLOB PRIMARY KEY NOT NULL,
  user_id BLOB REFERENCES users(id),       -- NULL = device with nobody PINed in
  device_id BLOB REFERENCES devices(id),   -- NULL = browser password session
  token_hash TEXT NOT NULL UNIQUE,
  kind TEXT NOT NULL,                      -- PASSWORD | DEVICE | PIN
  issued_at TEXT NOT NULL,
  expires_at TEXT NOT NULL,
  last_seen_at TEXT NOT NULL,
  revoked_at TEXT,
  CHECK (user_id IS NOT NULL OR device_id IS NOT NULL)
) STRICT;

restaurant_settings(
  id INTEGER PRIMARY KEY CHECK (id = 1),   -- singleton
  name TEXT NOT NULL,
  logo_path TEXT,
  address TEXT,
  phone TEXT,
  currency_code TEXT NOT NULL,
  currency_minor_units INTEGER NOT NULL,
  default_tax_rate_bp INTEGER NOT NULL,
  service_charge_rate_bp INTEGER NOT NULL,
  receipt_footer TEXT,
  timezone TEXT NOT NULL,
  service_day_cutoff_time TEXT NOT NULL,       -- 'HH:MM' local
  require_customer_order_approval INTEGER NOT NULL DEFAULT 1,
  business_hours_json TEXT,
  updated_at TEXT NOT NULL
) STRICT;
```

Indexes: `sessions(token_hash)` is already unique; add `sessions(expires_at)` for the sweep and `sessions(user_id)` for "revoke all sessions for this user".

### 2.3 One sessions table, three shapes

```
admin laptop      sessions(user_id=U, device_id=NULL, kind=PASSWORD)
floor tablet      sessions(user_id=NULL, device_id=D, kind=DEVICE)     long-lived
  + staff PIN     sessions(user_id=U,    device_id=D, kind=PIN)        short, auto-locks
```

One table, one lookup, one revocation path, one expiry sweep. A floor tablet carries two cookies and therefore two rows, which is exactly right: the device session must outlive staff coming and going.

The nullable `user_id` is not an accident of modelling. M4's kitchen display is a device with nobody PINed in that must still show tickets — a device-scoped, person-less function the schema expresses natively.

### 2.4 Two hash families, chosen for different reasons

| Secret | Entropy | Hash | Why |
|---|---|---|---|
| Session / device token | 32 bytes CSPRNG | SHA-256 | High entropy; Argon2 would add latency to every request for no gain |
| Password | Low | **Argon2id** | Offline-guessable |
| PIN | ~20 bits (6 digits) | **Argon2id** + lockout | Trivially guessable without lockout |
| Enrolment code | ~50 bits, 10 min TTL, single use | SHA-256 | High enough entropy, short-lived, rate-limited |

---

## 3. Flows

```
FIRST RUN
  boot with zero users → mint setup token in memory, log it to console
  GET  /api/v1/setup/status        → { needs_setup: true }
  POST /api/v1/setup {token, restaurant, admin}
       → creates first admin + restaurant_settings row
  route returns 404 permanently once any user exists
  token never touches disk; a restart mints a new one

PASSWORD
  POST /api/v1/auth/login {username, password}
       → sessions(kind=PASSWORD), HttpOnly cookie
  POST /api/v1/auth/logout          → revokes it
  GET  /api/v1/auth/me              → the current Actor, for the UI

DEVICE ENROLMENT
  admin:  POST /api/v1/devices {name}       → one-time code, 10 min TTL
  tablet: POST /api/v1/devices/enrol {code} → sessions(kind=DEVICE), HttpOnly cookie
  admin:  DELETE /api/v1/devices/{id}       → revoke; tablet drops to enrolment screen

PIN
  GET  /api/v1/auth/staff           → users with a PIN set, on a device session
  POST /api/v1/auth/pin {user_id, pin}  → sessions(kind=PIN)
  POST /api/v1/auth/lock            → revokes the PIN session; device session survives
```

`GET /api/v1/auth/staff` requires a valid device session. Without one it returns 401 — the staff roster is not public. It lists only users who are `active` and have a `pin_hash` set, so back-office accounts without a PIN never appear on a floor terminal's name picker.

### 3.1 Lifetimes

| Session | Idle | Absolute | Reasoning |
|---|---|---|---|
| `PASSWORD` | 12 h | 24 h | An office browser; a shift plus slack |
| `DEVICE` | — | 90 d | A terminal must never drop mid-service |
| `PIN` | **15 min** | 12 h | Auto-lock is the entire point of PIN switching |

Constants in `infra::session`, with a comment explaining each. Not configurable in M1 — nothing has asked for it, and every setting is a thing to get wrong.

`last_seen_at` updates on use, which is what makes idle timeout work. To avoid a write on every request, only update when the stored value is more than 60 seconds stale.

### 3.2 Lockout and rate limits

| Surface | Policy |
|---|---|
| PIN | 5 failures → lock that user 5 minutes; counter clears on success |
| Password | 10 failures per username in 15 minutes → 15 minute lockout |
| Enrolment code | 5 attempts per IP per 10 minutes |

PIN lockout is per-user rather than per-IP because a floor terminal has one IP and many users; locking the terminal would let one person's fat fingers stop the whole section.

**The two lockouts live in different places, deliberately.** PIN lockout is **persistent**, in `users.failed_pin_attempts` and `users.locked_until` — a 20-bit secret must not have its attempt counter cleared by restarting the server. Password and enrolment-code limits are **in-memory** in the rate limiter, keyed by username and by IP respectively; a restart clearing those is acceptable, because the underlying secrets are strong and the limiter exists to blunt online guessing rather than to be an audit record.

A roughly 40-line fixed-window limiter in `infra/ratelimit.rs` with an **injectable clock**. No new dependency: three endpoints need limiting, and an injectable clock makes the tests deterministic instead of sleep-based.

---

## 4. Authorization

MVP.md §6's permission list becomes a `const` enum in `core::permission`, so a typo is a compile error rather than a silent grant.

```rust
pub struct Actor {
    pub user: Option<AuthUser>,     // None = device present, nobody PINed in
    pub device: Option<DeviceRef>,
    pub permissions: PermissionSet,
}

require!(actor, Permission::UsersManage)?;
```

`Actor` is an axum `FromRequestParts` extractor reading both cookies. **A missing or expired session yields an empty `Actor`, not an error** — routes decide what that means, and a public route stays public without special-casing.

**401 and 403 are deliberately distinct.** No user is 401, and the UI redirects to login. A user without the permission is 403, and the UI says so. Collapsing them makes the interface lie in one direction or the other.

### 4.1 Seeded roles

The six roles from MVP.md §5 are seeded with `is_system = 1` and cannot be deleted: Admin, Manager, Steward, Cashier, Kitchen, Bar. Admin holds every permission. Admins can create further roles.

### 4.2 Permission drift fails startup

The `permissions` table is seeded from the enum at boot. **If the table holds a code the enum does not define, startup fails.** Without that, the const enum only prevents typos in new code while the database quietly accumulates stale codes that roles still reference — and a role granting a permission the code no longer checks is a silent hole.

---

## 5. Audit attribution

M0 made the `AuditRecord` a required argument to `write_tx`. M1 makes the attribution real:

```rust
AuditRecord::by_actor("user.create", "user", &actor)
```

replaces M0's manual `.by(uuid)`, so attribution stops being something each feature has to remember. `user_id` stays nullable: first-run setup has no actor, and that is the only legitimate case.

The audit viewer at `/admin/audit` is in M1 rather than M8 because M1's headline claim is that every mutation is attributed and recorded — and a claim nobody can look at is a claim nobody checks.

## 6. Ruling J closes here

M0 deferred write-pool encapsulation with a CI guard standing in for a type. M1 finishes it:

- `Db.write` becomes private, with a `pub(crate)` accessor only `infra/write.rs` uses.
- Test fixtures get `#[cfg(debug_assertions)] pub fn fixture_pool()` — conspicuously named, absent from release builds. The same technique that hid `dev_ping` in M0.
- `scripts/check-write-path.sh` stays. It then guards a boundary the compiler mostly enforces, rather than standing in for one.

---

## 7. UI

The web app currently has no router. M1 adds TanStack Router — the architecture design names it, and routing cannot be deferred further — plus a `useSession()` hook over `GET /api/v1/auth/me` driving redirects.

```
/setup      first-run only; disappears once a user exists
/login      username + password
/terminal   enrolment code → name picker → PIN pad → lock screen
/admin
  /users    list, create, edit, deactivate; assign role; set password or PIN
  /roles    role list + permission matrix over the enum
  /devices  list, name, generate enrolment code, revoke
  /settings restaurant config: currency, tax, service charge, timezone, service-day cutoff
  /audit    read-only, paginated
```

**No dashboard.** There is nothing to put on one until M8 has revenue and order counts, and an empty dashboard is worse than none.

---

## 8. Testing

Per-feature tests as usual. Three carry disproportionate weight:

- **Every mutating route requires a permission.** A test enumerates the router's routes and asserts each non-`GET` route rejects an empty `Actor` with 401. This is the automated form of "did someone forget a `require!`" — precisely the failure the architecture design's router-separation argument leaves open between staff roles.
- **Permission drift fails startup.** Insert a code the enum does not define; assert the server refuses to boot.
- **Session boundaries**: expired yields an empty `Actor`, not an error; revoked is rejected; revoking a PIN session leaves the device session intact; lockout releases exactly on time, via the injectable clock and no sleeps.

Frontend: Vitest over the session-state logic and the redirect decisions, as pure functions, following M0's `eventStream` reducer pattern.

---

## 9. Task shape

Roughly 22 tasks:

```
core          1  Permission enum + PermissionSet
schema        2  migration: users, roles, permissions, role_permissions, settings
              3  migration: devices, device_enrolments, sessions
infra         4  crypto: Argon2id for secrets, SHA-256 for tokens
              5  rate limiter with injectable clock
              6  permission seeding + drift check at boot
              7  session store: create, resolve, revoke, sweep
              8  Actor extractor + require! macro
              9  write-pool privatisation (Ruling J)
routes       10  first-run setup
             11  password login / logout / me
             12  device enrolment
             13  PIN in / lock + lockout
             14  restaurant settings
             15  users + roles CRUD
             16  audit attribution + by_actor; route-guard coverage test
web          17  TanStack Router + useSession + login
             18  setup page
             19  admin shell + users + roles
             20  devices + settings + audit viewer
             21  terminal: enrolment, name picker, PIN pad, lock screen
             22  acceptance
```

---

## 10. Carried forward from M0

Open items M1 should address or explicitly re-defer:

- **Ruling M (M0):** the WebSocket topic filter is tested at the predicate, not at its wiring into `pump`. Deleting the filter line would fail no test. M1 should add a socket-level test publishing a `Topic::Table` envelope and asserting a staff connection does not receive it.
- **M0 deferred minor:** `npm audit` reports 5 vulnerabilities in transitive dev dependencies; `vitest ^2 → ^4` clears them.
- **M0 deferred minor:** the API drift gate does not catch a brand-new untracked generated file. Add `git status --porcelain web/src/lib/api` to `check-api`.
- **Architecture design §7** should be amended to record the two-path authentication decision from §1.1 above.
