# grubsi — Architecture Design

**Date:** 2026-08-31
**Status:** Approved · revised after design audit
**Source spec:** [docs/MVP.md](../../MVP.md)
**Scope:** System architecture and milestone decomposition. Individual milestones get their own specs and implementation plans; this document is the substrate they reference.

---

## 1. Decisions

| Area | Decision |
|---|---|
| Deployment | One Rust binary on a dedicated LAN box, serving all four web UIs as embedded assets. No Tauri in the MVP. |
| Backend structure | Three crates — `core` (pure), `escpos`, `server` (feature-sliced) |
| Frontend | React 19 + Vite + TanStack Router/Query + Tailwind; one app, route groups per surface |
| Database | SQLite, WAL, split read/write pools |
| Order model | Check → Ticket → Item |
| Customer QR orders | Require steward approval before firing |
| Pricing | Tax-exclusive prices; per-item tax rate; service charge included in the taxable base |
| Printing | ESC/POS over TCP:9100, with a fake sink for CI |
| Time | All timestamps stored UTC; service day boundary is a configurable cutoff |
| Implementation mode | Solo developer directing AI agents |

Each decision's reasoning appears in the relevant section below.

### 1.1 Rejected alternatives

**Tauri desktop app (MVP.md §4.2, §51).** Dropped from the MVP. It adds a second build target, a second UI stack, and a packaging/update story, to deliver a screen the admin can already open in a browser on the same LAN. Tauri remains available later as a thin shell around the same admin routes.

**Twelve-crate workspace (MVP.md §52).** Crates in Rust are compile units, not organizational units. `orders` → `pricing` → `menu` → `tables`, with `audit` depending on all of them, is a dependency graph that costs real time to untangle, and every boundary crossing needs `pub` types and re-exports. Module boundaries plus `pub(crate)` give the same discipline. Extracting a crate later is mechanical; merging one back is not.

**Event sourcing.** Would give MVP.md §36's audit trail and §42's authoritative-server property for free, and make order history replayable. Rejected for the MVP: it roughly doubles per-feature work (command + event + projection instead of an `UPDATE`), makes ad-hoc reporting queries substantially harder, and is a pattern agent-written code gets subtly wrong far more often than CRUD. Revisit in Phase 3 if cloud sync lands.

---

## 2. Process model

One `grubsi-server` binary on one Tokio runtime.

```
/api/v1/*   REST, JSON — authenticated staff
/api/public/*  REST, JSON — customer QR (separate router, see §7)
/ws         WebSocket — one socket per client, topic subscriptions
/*          embedded SPA assets (rust-embed), fallback → index.html
```

Background tasks spawned at boot:

| Task | Responsibility |
|---|---|
| Per-printer dispatcher | One task + mpsc queue **per printer** — a thermal printer cannot interleave two jobs |
| Printer health prober | Periodic TCP connect; emits `PRINTER_STATUS_CHANGED` |
| mDNS responder | Advertises `grubsi.local` (MVP.md §44) |
| Shutdown handler | Drains in-flight print jobs, checkpoints the WAL |

### 2.1 SQLite access

Two pools:

- **Write pool: exactly 1 connection.** Serializes all writers.
- **Read pool: N connections.**

Pragmas: `journal_mode=WAL`, `foreign_keys=ON`, `synchronous=NORMAL`, `busy_timeout=5000`.

The single-writer pool costs nothing at restaurant scale (a busy night is a few writes per second) and structurally eliminates `SQLITE_BUSY` — an intermittent, hard-to-reproduce failure class that agent-written concurrent code walks into repeatedly.

**The write connection is therefore a global lock.** No CPU-bound or network work may happen inside a write transaction. ESC/POS rendering, password hashing, QR image generation and image resizing all happen *before* the transaction opens; the transaction only writes finished bytes. Violating this serializes the entire restaurant behind one slow operation.

### 2.2 Identifiers and time

UUIDv7 stored as `BLOB(16)` for all entity primary keys. Monotonic, so index locality stays good, and it satisfies MVP.md §45's requirement that customer URLs not expose internal database IDs without maintaining a parallel public-id column.

QR tokens are 32 bytes of CSPRNG output, base64url-encoded, stored with a unique index, and individually revocable.

**All timestamps are stored in UTC**, without exception, and converted to `restaurant_settings.timezone` only at the presentation and reporting boundary. Mixed-zone storage is unfixable retroactively.

**The service day is a configurable cutoff**, `restaurant_settings.service_day_cutoff_time`, defaulting to 04:00 local — restaurants that trade past midnight need the night's takings to land on the night they started. Human-facing check numbers (`#1042`) are a per-service-day sequence, so **this is required by M4, not M8**, and every report's date bucketing derives from the same function.

### 2.3 The write path

Every mutation goes through one helper:

```rust
write_tx(
    audit: AuditRecord,
    idem: Option<IdempotencyKey>,
    f: impl FnOnce(&mut Tx) -> Result<T>,
) -> Result<(T, Vec<DomainEvent>)>
```

Three things are enforced by the signature rather than by discipline:

- **The audit record is a required argument.** You cannot commit a mutation without one, so MVP.md §36's audit trail cannot be forgotten by a feature written six milestones from now.
- **Idempotency is available at the same seam** where it is needed, rather than bolted onto individual endpoints (see §4.6).
- **Domain events are returned, not published**, which is what makes §6's after-commit rule structurally difficult to violate.

§3 argues that `core`'s boundary works because the compiler enforces it. Audit logging is the most cross-cutting concern in the system and the easiest to omit silently; it earns the same treatment.

---

## 3. Crate layout

```
grubsi/
├── Cargo.toml                  workspace
├── crates/
│   ├── core/                   NO sqlx, axum, or tokio in Cargo.toml
│   │   └── src/
│   │       ├── money.rs        minor units, basis points, rounding
│   │       ├── pricing.rs      the §5 calculation
│   │       ├── check.rs        check state machine
│   │       ├── ticket.rs       ticket + item state machines
│   │       ├── permission.rs   const enum of all permission codes
│   │       └── ids.rs
│   ├── escpos/
│   │   └── src/
│   │       ├── document.rs     semantic ticket document
│   │       ├── render.rs       ticket → Document
│   │       ├── encode.rs       Document → bytes, per printer profile
│   │       └── transport/{tcp,file,fake}.rs   -- feature-gated
│   └── server/
│       └── src/
│           ├── infra/          db · ws · print_queue · auth · error
│           ├── features/
│           │   ├── auth/  menu/  tables/  stations/  printers/
│           │   ├── checks/  tickets/  kds/
│           │   ├── billing/  discounts/
│           │   └── reports/  admin/  audit/
│           └── main.rs
├── migrations/                 sqlx, versioned
└── web/                        React app, built into server binary
```

Each feature folder owns its full stack — `routes.rs`, `service.rs`, `repo.rs`, `events.rs`, `tests.rs` — and shares only `infra/` and `core`. An agent implementing one feature opens one folder and sees the routes, the queries, and the tests together, and cannot reach into another feature's internals.

**The `core` boundary is the important one.** `core` has no I/O dependencies in its `Cargo.toml`, so the code where mistakes are most expensive and least visible — money arithmetic, tax ordering, state transitions, permission checks — physically cannot compile a database call or an HTTP handler into it. That constraint is worth more than review discipline.

`escpos` gates its transports behind a Cargo feature so that `render` and `encode` stay free of a runtime dependency and remain testable in isolation.

---

## 4. Domain model

Rates are integer **basis points**. Money is integer **minor units** (MVP.md §40). No floating point anywhere in the money path.

Every `_snapshot` column is named that way deliberately: it makes MVP.md §18's historical-accuracy rule visible at the point of use, so nobody joins a historical ticket back to the live menu.

```sql
restaurant_settings   -- singleton row
  currency_code, currency_minor_units, default_tax_rate_bp,
  service_charge_rate_bp, receipt_footer, timezone, business_hours_json,
  service_day_cutoff_time, require_customer_order_approval,
  name, logo_path, address, phone

users(username, display_name, password_hash, role_id, active)
roles(name, is_system)
permissions(code)                     -- seeded from core::Permission at boot
role_permissions(role_id, permission_code)
sessions(user_id, token_hash, expires_at, revoked_at)
table_sessions(check_id, token_hash, expires_at, revoked_at)

floors(name, display_order)
tables(floor_id, name, capacity, pos_x, pos_y, width, height, rotation,
       out_of_service)
table_qr_tokens(table_id, token UNIQUE, active, revoked_at)

menu_categories(parent_id, name, display_order, active)
menu_items(category_id, name, description, price_minor, image_path,
           tax_rate_bp NULL, station_id, available, display_order, active)
modifier_groups(name, min_select, max_select, required)
modifier_options(group_id, name, price_delta_minor, display_order)
menu_item_modifier_groups(menu_item_id, modifier_group_id, display_order)

stations(name, kind)                  -- KITCHEN | BAR | RECEIPT
printers(station_id, name, host, port, paper_width, codepage, active,
         priority)                    -- fallback = next active by priority
print_jobs(printer_id, kind, payload_blob, status, attempts, last_error,
           is_reprint, maybe_duplicate, requested_by, ticket_id, check_id)

checks(number, table_id, status, opened_by, opened_at, closed_at,
       subtotal_minor, discount_minor, service_charge_minor, tax_minor,
       total_minor, applied_minor, refunded_minor, merged_into_check_id)
tickets(check_id, source_table_id, seq, source, status, created_by,
        approved_by, fired_at, idempotency_key)
ticket_items(ticket_id, menu_item_id, name_snapshot, unit_price_minor,
             tax_rate_bp_snapshot, station_id_snapshot, quantity, notes,
             status, discount_minor, discount_reason,
             line_subtotal_minor, voided_at, voided_by)
ticket_item_modifiers(ticket_item_id, modifier_option_id, name_snapshot,
                      price_delta_minor_snapshot)

discounts(name, type, value, scope, scope_ref_id, starts_at, ends_at, active)
check_discounts(check_id, discount_id, apply_seq, name_snapshot,
                type_snapshot, value_snapshot, amount_minor,
                applied_by, applied_at)

payments(check_id, method, tendered_minor, applied_minor, reference,
         received_by, idempotency_key, voided_at, voided_by)
refunds(payment_id, amount_minor, reason, refunded_by, idempotency_key,
        created_at)

customer_requests(table_id, check_id, kind, status, acknowledged_by,
                  completed_at)

audit_logs(user_id, action, entity_type, entity_id, before_json, after_json,
           created_at)
```

Unique indexes: `(check_id, idempotency_key)` on `payments`, `(check_id, idempotency_key)` on `tickets`, `(payment_id, idempotency_key)` on `refunds`.

### 4.1 Why Check → Ticket → Item

MVP.md contradicts itself: §20 defines a linear per-order lifecycle (`DRAFT → PLACED → … → SERVED → BILLED → PAID`), but §22 has a steward appending items to an existing order with only the new items printing, §33 wants split payments, and §35 wants merged tables whose items stay traceable to their source table. A single flat `order` row cannot be both `SERVED` and `PREPARING`, and merging loses provenance.

Three levels, each with one job:

- **Check** — the billing unit. Opens when the table is seated, closes when paid. Owns totals, discounts, payments.
- **Ticket** — one *fire*: the immutable batch of items sent to stations at a moment in time. Owns the kitchen lifecycle. This is what a KOT reprints.
- **Item** — a line, with its own prep status and its own price snapshot.

Everything else falls out:

| Operation | Implementation |
|---|---|
| Add items later | A new ticket on the same check |
| Transfer table | `UPDATE checks SET table_id = ?` |
| Merge tables | Reassign tickets to the surviving check; set `merged_into_check_id` on the other. §35's traceability is already satisfied by `tickets.source_table_id`. |
| Split payment | Many `payments` rows per check |
| Partial readiness | Per-ticket and per-item status; no state machine has to run backwards |
| Reprint | Re-emit the ticket's stored bytes with `is_reprint = true` |

### 4.2 State machines

```
check     OPEN → BILL_REQUESTED → PARTIALLY_PAID → PAID → CLOSED
            ↘ VOIDED                                 ↘ REFUNDED

ticket    PENDING_APPROVAL → FIRED → ACCEPTED → PREPARING → READY → SERVED
                          ↘ REJECTED                              ↘ CANCELLED
```

Tickets created by a steward skip `PENDING_APPROVAL` and start at `FIRED`. Transitions are validated in `core` (MVP.md §20, §42) and are the only way status changes.

A check reaches `PAID` when `applied_minor >= total_minor`, and `REFUNDED` only when refunds cover the full applied amount; a partial refund leaves the check `PAID` with `refunded_minor` recorded against it. Reopening a `CLOSED` check is not a transition — it is a new, separately audited and separately permissioned action.

### 4.3 Void versus refund

These are different operations and conflating them corrupts the day's takings:

- **Void** erases a mistake before settlement. `payments.voided_at` removes the row from every total as though it never happened.
- **Refund** moves money back after settlement. It is an additive `refunds` row that never mutates the original payment, so the day's gross and its refunds are both reportable.

Item voids are separate again: `ticket_items.voided_at` excludes a line from pricing entirely. Voiding an item on a ticket that has already fired emits `TICKET_STATUS_CHANGED` so the station is told to stop cooking it — the kitchen is not a database, and it will not notice a row change on its own.

### 4.4 Table status is derived, not stored

MVP.md §9 lists eight table states. Seven are a pure function of the table's open check — `AVAILABLE` = no open check, `OCCUPIED` = open check exists, `BILL_REQUESTED` = check status, and so on. Only `OUT_OF_SERVICE` is independently stored.

Storing the rest would create two sources of truth that drift the first time a transaction half-fails or an event is missed.

### 4.5 Station is first-class; items route to stations, never to printers

MVP.md §23 (routing) and §24 (printers) are separate concerns. A menu item names a **station**; a station has zero or more printers *and* a KDS view.

Printers within a station are ordered by `priority`, and fallback is simply "the next active printer in this station". There is exactly one fallback mechanism, not a routing rule plus a peer-to-peer backup pointer.

This is what lets a dead kitchen printer degrade to "the KDS still works" instead of halting service, and it means adding a second kitchen printer is configuration rather than a menu-wide edit.

### 4.6 Idempotency

Firing a ticket, recording a payment, and issuing a refund all carry a client-generated `idempotency_key`, enforced by a unique index. A retried request returns the original result instead of creating a second row.

This is not defensive over-engineering. A steward taps *Record payment*, the response is slow on a congested restaurant Wi-Fi, and they tap again — without a key, the check is now overpaid and closes. It is the most likely money bug in the entire system, and integration tests do not catch it because the fault is a client retry, not a server defect.

### 4.7 Check totals are a cached projection

`core::pricing` recomputes totals from scratch and the service writes the result inside the same transaction as the mutation. Totals are never incrementally adjusted — incremental money arithmetic drifts, and the drift is invisible until a customer disputes a bill.

---

## 5. Pricing

Tax-exclusive menu prices. Per-item tax rate, falling back to the restaurant default — MVP.md §12's item-level `tax` field, which matters because alcohol is commonly taxed differently from food.

The calculation is an ordered cascade. Each step is named, and each is where a rounding decision lands:

```
1.  line subtotal   (unit_price + Σ modifier deltas) × quantity
2.  line discount   − ticket_items.discount_minor        per-line comps
3.  subtotal        Σ non-voided lines
4.  check discounts applied in apply_seq: percentages first,
                    then fixed amounts, each against the running
                    discounted subtotal, floored at zero
5.  service charge  on the discounted subtotal
6.  allocation      service charge split across tax groups pro-rata
                    by discounted line value
7.  tax             per group, on (group value + allocated service charge)
8.  total           discounted subtotal + service charge + Σ tax
```

Worked example:

```
2 × Chicken Burger  @ 850                    1700
1 × Beer            @ 600                     600
  + Extra Cheese    @ 100                     100
                                         ────────
Subtotal                                     2400
Discount (10% off food)                      −180
                                         ────────
Discounted subtotal                          2220
Service charge 5%                             111
                                         ────────
Taxable base                                 2331
  food   10%  on 1620 + 81  →                 170
  liquor 15%  on  600 + 30  →                  95
                                         ────────
TOTAL                                        2596
```

Rounding is half-up, applied once, at each named line. All values are integer minor units.

Five rules the cascade relies on, stated explicitly because each is a place where two readings are possible:

1. **Tax applies to the service charge.** MVP.md §15's formula (`Item + Modifiers − Discounts + Service Charge + Tax`) does not say; this document resolves it as yes.
2. **Discounts reduce the taxable base**, and a scoped discount reduces only the lines in its scope. In the example, the 10%-off-food discount reduces the food tax group from 1800 to 1620 and leaves the liquor group untouched.
3. **The service charge is allocated across tax groups pro-rata by discounted line value**, because a single service charge must be split before differing tax rates can apply to it. Above: food takes 1620/2220 of 111 = 81, liquor takes 600/2220 of 111 = 30. Allocation remainders go to the largest group so the parts always sum to the whole.
4. **Multiple check-level discounts apply in `apply_seq` order, percentages before fixed amounts**, each computed against the running discounted subtotal rather than the original. Percentages therefore do not compound against each other's output in an order the operator did not choose, and the discounted subtotal is floored at zero.
5. **Modifiers inherit their parent item's `tax_rate_bp_snapshot`.** A modifier has no tax rate of its own; "+ Extra Cheese" on a burger is taxed as food even if the check also contains liquor lines.

Voided items are excluded from every step, not zeroed — a voided line must not affect a pro-rata allocation.

The entire calculation is one pure, side-effect-free function in `core::pricing`. It is the highest-consequence code in the system.

---

## 6. Real-time

One `tokio::sync::broadcast` hub.

**Events publish after commit, never inside the transaction.** §2.3's `write_tx` returns events rather than publishing them; the caller publishes once the transaction lands. Broadcasting inside a transaction eventually broadcasts state that gets rolled back.

Topics:

| Topic | Subscribers |
|---|---|
| `staff` | All authenticated staff |
| `station:{id}` | KDS screens — only that station's items |
| `table:{id}` | Customer QR — **only their own table** |
| `check:{id}` | Clients viewing one check |

**Topic authorization happens at connect, not at subscribe.** The session determines the set of topics that socket may ever join; the client then selects from that set and cannot name a topic outside it. A customer session resolves to exactly one `table:{id}` and one `check:{id}`. Declaring customer scoping a security boundary is worth nothing without this — a subscribe-time string check is exactly the kind of guard that gets refactored away.

Envelope: `{ boot_id, seq, type, topic, payload, at }`. On reconnect the client sends its `last_seq`; a gap, or a changed `boot_id`, triggers a full refetch. This delivers MVP.md §53's "recovers cleanly after server restart", and it repairs the one hole in commit-then-publish — if the process dies between commit and broadcast, the missed event is recovered on reconnect rather than lost.

A slow consumer yields `RecvError::Lagged` from the broadcast channel. **`Lagged` maps to the same full refetch as a sequence gap** — it is not an error and not a disconnect. The default reaction to it is wrong in both directions, so it is specified here.

Event types are MVP.md §29's list, adjusted to the Check/Ticket model: `CHECK_OPENED`, `CHECK_UPDATED`, `CHECK_CLOSED`, `TICKET_PENDING`, `TICKET_FIRED`, `TICKET_STATUS_CHANGED`, `TABLE_UPDATED`, `PAYMENT_CREATED`, `PAYMENT_REFUNDED`, `CUSTOMER_REQUEST`, `PRINTER_STATUS_CHANGED`, `PRINT_JOB_FAILED`.

### 6.1 Client integration

A WebSocket event only ever calls `queryClient.invalidateQueries(...)`. It never patches the cache from the payload.

One line per event instead of a client-side reducer that duplicates server logic, and MVP.md §42's "server is authoritative" holds by construction rather than by discipline.

---

## 7. Authentication

Two entirely separate schemes. Conflating them is the classic failure in this kind of system.

| | Staff | Customer |
|---|---|---|
| Credential | Username + Argon2id password | QR token → minted table session |
| Token | 32-byte opaque, stored hashed | Short TTL, **bound to a check** |
| Cookie | `HttpOnly; SameSite=Lax` | `HttpOnly; SameSite=Lax` |
| Identity | User + role | None |
| Router | `/api/v1/*` | `/api/public/*` |
| Rate limit | Standard | Per-session, aggressive |

Customers are kept out of staff routes by **router separation**, not by a permission check. A forgotten permission guard on a staff route is then a privilege bug between staff roles; it can never expose the API to the dining room.

Server-side sessions rather than JWT: instant revocation matters when a staff device walks off, and there is no scaling argument for stateless tokens on a single LAN box.

**A customer session is bound to a check, not to a table**, and is revoked when that check closes. Binding to the table leaves the departing party's phone holding a live session for the table the next party is now sitting at — they could order onto strangers' bills. Scanning the QR again simply mints a new session against the new check.

A table's QR only accepts orders while the table is not `OUT_OF_SERVICE` and its token is `active`.

### 7.1 Permissions

MVP.md §6's list becomes a `const` enum in `core::permission`. A typo'd permission code is then a compile error rather than a silent grant. Roles are database rows mapping to those codes; the six roles in MVP.md §5 are seeded, and admins can create more.

**The `permissions` table is seeded from the enum at boot, and startup fails if the table contains a code the enum does not define.** Without reconciliation the const enum only prevents typos in new code while the database quietly accumulates stale codes that roles may still reference.

Route guard: `require!(ctx, Permission::OrdersVoid)`.

---

## 8. Printing

`print_jobs` is a durable queue in SQLite, **written in the same transaction as the domain mutation**. If the server dies a millisecond after a ticket fires, the KOT is still queued.

```
enqueue (in tx) → PENDING → PRINTING → PRINTED
                                    ↘ FAILED → retry(backoff) → DEAD
                                                    ↘ next printer by priority

on boot: rows stuck in PRINTING → PENDING, attempts + 1, maybe_duplicate = true
```

Failures emit `PRINT_JOB_FAILED`, surfacing MVP.md §26's `[Retry] / [Print on Backup]` banner.

**ESC/POS bytes are rendered at enqueue time and stored on the job**, not re-rendered at print time. A reprint is therefore byte-identical to the original, and a menu edit next week cannot retroactively change what a KOT said. Reprints are a new job with `is_reprint = true` and a REPRINT band (MVP.md §25).

Per §2.1, **rendering happens before the transaction opens** — the transaction inserts a finished blob and nothing more. Rendering inside it would serialize every write in the system behind ESC/POS encoding.

The `escpos` crate splits `render(ticket) → Document` from `encode(Document, profile) → Vec<u8>`. The KDS and the developer preview page consume the same `Document`, so what is on screen and what is on paper cannot diverge.

Transports behind one trait:

```rust
trait TicketSink {
    async fn send(&self, job: &PrintJob) -> Result<()>;
}
```

- `EscPosTcpSink` — production, TCP:9100
- `FileSink` — local development, dumps raw bytes
- `FakePrinter` — in-process listener for tests, with modes: `ok`, `refuse`, `hang`, `die_midjob`, `offline`

Real network printers are available for development, so the ESC/POS command set and paper-width/codepage handling get validated against hardware early. The fake sink still exists because CI has no printers and because MVP.md §26's failure paths cannot be tested any other way.

---

## 9. Errors

One `AppError` in `server::infra::error`, carrying:

- a machine-readable `code`
- a **mandatory** user-facing `message`
- an internal detail that goes only to `tracing`

MVP.md §48 is enforced by the type: it is not possible to construct an error that leaks `SQLITE_CONSTRAINT_UNIQUE` to a steward's tablet.

`core` returns typed domain errors (`PricingError`, `TransitionError`, `PermissionDenied`) and never knows HTTP exists; conversion happens at the `server` boundary.

Structured logging via `tracing` at the levels and for the events in MVP.md §49.

---

## 10. Testing

Given that agents write most of the code, tests are the primary safety net and are designed rather than assumed.

| Layer | Strategy |
|---|---|
| `core` | Golden pricing table of ~40 scenarios crossing discounts × per-item tax rates × service charge × rounding boundaries, including multi-discount ordering and per-line comps. Property tests: totals reconcile, parts sum to the whole, nothing goes negative. State machine tests enumerate legal and illegal transitions. |
| `escpos` | Golden byte files per printer profile. |
| `server` | Integration tests against a real temp SQLite **file** — not `:memory:`, because WAL semantics differ — driving the actual axum router via `tower::ServiceExt::oneshot`. |
| Idempotency | Every idempotent endpoint has a test that issues the identical request twice and asserts one row and one identical response. |
| End-to-end | At least one test walks MVP.md §3's entire arc: seat → customer orders → steward approves → fire → assert the fake printer received the right bytes → ready → bill → split payment → close → assert it appears in the day's report. |
| Contract | OpenAPI generated from the routes via `utoipa`; TypeScript client generated from that in CI. The API contract is a build artifact, so backend and frontend work cannot drift apart silently. |

---

## 11. Milestones

MVP.md is not one implementation plan. Each milestone below gets its own spec and plan, referencing this document.

The sequencing principle is **walking skeleton first**: get one trivial request travelling the entire stack before building any feature on it, then widen.

### M0 — Walking skeleton

No features. Workspace and three crates; CI; migrations and the dual pool; the `write_tx` helper; axum plus embedded React with SPA fallback; `AppError`; `tracing`; one WebSocket round-trip; OpenAPI → TypeScript codegen; the integration test harness (temp database + router + fake printer).

**Done when:** `cargo run`, open the app from another device on the LAN, and watch a server-pushed event change the page.

### M1 — Identity and configuration

Users, roles, permissions and boot-time seeding, sessions, login. `restaurant_settings` including the service-day cutoff. **The audit log write path.** Admin shell UI.

Audit logging is in M1 rather than M8 because it is cross-cutting — every later feature writes to it. Retrofitting audit across eight finished features is the most predictable way to lose a week on this project.

### M2 — Restaurant structure

Floors and tables, including the layout editor (position, size, rotation). QR tokens and QR image generation. Stations, printer CRUD, health probe. Menu categories, items, modifier groups and options, availability.

### M3 — Pricing engine

`core` only, no UI. Money and rate types, the discount model, the full §5 cascade, the golden test table.

Its own milestone because it is pure, it is the highest-consequence code in the system, and it must be provably correct before M4 and M6 consume it. Isolating it means 40 golden cases can be reviewed in one sitting instead of a rounding bug surfacing in a receipt three months later.

### M4 — The core loop

*This is the actual MVP.*

Checks, tickets, ticket items. Service-day check numbering. Steward flow: floor view → open check → build ticket → fire. Station routing, print job enqueue, ESC/POS KOT/BOT, per-printer dispatcher, retry and priority fallback. KDS with accept / preparing / ready and per-station filtering. WebSocket events with connect-time topic authorization.

**Done when:** a steward seats a table, fires an order, paper comes out of the right printer, and the KDS shows it.

### M5 — Customer QR ordering

Public router, check-bound table session, menu browsing, cart, place order. Steward approval queue. Live order status for the customer. `customer_requests` (call waiter, request bill).

After M4, not alongside it. Customer ordering is a second *input* into a pipeline that already works; built second it reuses the ticket path, built in parallel it produces two order pipelines that drift.

### M6 — Billing and payments

Discount application and ordering, per-line comps, bill generation and printing, payments including splits, tender versus applied amounts and change due, voids, refunds, partially-paid state, closing a check, receipt printing and reprinting.

### M7 — Table operations

Transfer and merge. After billing, because merge semantics are only testable once checks carry totals and payments.

### M8 — Reporting and operations

Dashboard. Sales, orders, payments, discounts, refunds and staff reports, bucketed by service day. CSV export. System health screen with connected device counts. Backup. mDNS and LAN address display.

Backup uses `VACUUM INTO` or the SQLite Online Backup API — **never a file copy**, which produces a corrupt database when WAL is active, and produces it silently.

### 11.1 Cut list

If time compresses, these go first, in this order:

1. **Table merge (M7)** — the most complex feature relative to how often it is used.
2. **Drag/resize/rotate layout editor (M2)** — ship first as a grid with numeric position fields.
3. **Happy-hour time windows (MVP.md §16)** — already marked optional in the source spec.
4. **Menu category nesting** — cap the UI at two levels even though `parent_id` allows more.

---

## 12. Deferred audit findings

The design audit of 2026-08-31 raised five further items, deliberately left open rather than resolved here:

- **M2 is roughly twice the size of any other milestone.** Splitting it into M2a (floors, tables, QR, stations, printers) and M2b (menu, categories, modifiers, availability) is probably correct, but changes the milestone numbering that the rest of this document references.
- **The cut list mostly names free cuts.** Table merge is ranked first but is already built last, so cutting it relieves no schedule pressure. A useful cut list would name work in M2 and M8.
- **M0's definition of done omits the development loop.** With `rust-embed`, a React change requires a Rust rebuild, and a cold `cargo build` fails outright because `web/dist` does not exist. M0 needs a `dev` feature that serves assets from disk and proxies to Vite, and a build that succeeds on a fresh clone.
- **The HTTPS stance is unrecorded.** MVP.md §45 asks for "HTTPS where practical". Over LAN HTTP, cookies cannot carry `Secure` and the app has no secure context. This is an acceptable trade for the MVP but should be a written decision.
- **MVP.md §47's connected-device counts** need a live connection registry that nothing in this design currently provides.
