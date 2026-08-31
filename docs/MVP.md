# Restaurant Management System — MVP Specification

**Project Status:** MVP
**Version:** 0.1
**Architecture:** Local-first / LAN-based
**Primary Backend:** Rust
**Database:** SQLite
**Primary Goal:** Provide a reliable, easy-to-use restaurant management system covering table management, QR ordering, steward ordering, kitchen/bar order processing, thermal printing, billing, and basic analytics.

---

# 1. Product Overview

The application is a **local-first restaurant and bar management system** designed to operate primarily over the restaurant's local network.

The system will provide:

* Admin desktop application
* Steward web application
* Customer QR ordering web application
* Kitchen/bar order management
* KOT/BOT printing
* Thermal receipt printing
* Table and floor management
* Menu and pricing management
* Discounts
* Billing and payments
* User and permission management
* Basic analytics and reporting

The system should continue operating when the restaurant's Internet connection is unavailable.

Internet access is optional for the MVP and should not be required for normal restaurant operations.

---

# 2. Core Architecture

```text
                         REST / WebSocket
                                │
                                ▼
                    ┌──────────────────────┐
                    │      RUST SERVER     │
                    │                      │
                    │ Business Logic       │
                    │ REST API             │
                    │ WebSockets            │
                    │ Authentication       │
                    │ Printing              │
                    │ SQLite Database       │
                    └──────────┬───────────┘
                               │
              ┌────────────────┼─────────────────┐
              │                │                 │
              ▼                ▼                 ▼
       ┌─────────────┐  ┌─────────────┐  ┌─────────────┐
       │ Admin       │  │ Steward     │  │ Customer    │
       │ Desktop     │  │ Web App     │  │ QR Web App  │
       └─────────────┘  └─────────────┘  └─────────────┘
                               │
                               ▼
                       Restaurant LAN
                               │
                 ┌─────────────┼─────────────┐
                 ▼             ▼             ▼
            Kitchen        Bar Printer   Receipt Printer
            Printer
```

---

# 3. MVP Goals

The MVP must allow a restaurant to perform the following complete workflow:

```text
Configure Restaurant
       ↓
Create Floor / Tables
       ↓
Create Menu
       ↓
Configure Printers
       ↓
Customer scans QR
       ↓
Customer views menu
       ↓
Customer places order
       ↓
Order appears to Steward
       ↓
Order sent to Kitchen / Bar
       ↓
KOT / BOT printed
       ↓
Food / Drinks prepared
       ↓
Order served
       ↓
Bill generated
       ↓
Payment recorded
       ↓
Receipt printed
       ↓
Order closed
       ↓
Sales included in Analytics
```

The same workflow must also support orders entered directly by a steward.

---

# 4. Applications

The MVP consists of three primary clients and one backend.

## 4.1 Rust Server

The central application responsible for:

* API
* Authentication
* Database
* Business logic
* Order management
* Table management
* Menu management
* Pricing
* Discounts
* Printing
* Reporting
* Real-time events
* WebSocket connections

The server is the authoritative source of restaurant state.

---

# 4.2 Admin Desktop Application

The admin application is used for restaurant configuration and management.

### MVP functionality

* Login
* Dashboard
* Restaurant settings
* Floor/table management
* Menu management
* Pricing
* Discounts
* Printer configuration
* User management
* Role/permission management
* Order management
* Billing
* Basic reports
* System status

---

# 4.3 Steward Web Application

The steward application runs in a browser on devices connected to the restaurant LAN.

Example:

```text
http://restaurant.local/steward
```

### MVP functionality

* Login
* View floor/table layout
* View table status
* Open table
* Create order
* Add/remove items
* Add modifiers
* Add order notes
* Send order
* View order status
* Request KOT/BOT reprint
* Transfer table
* Merge tables
* Request bill
* Process payment
* Print receipt

---

# 4.4 Customer Web Application

Customers access the menu by scanning a QR code assigned to their table.

Example:

```text
http://restaurant.local/order/<table-token>
```

### MVP functionality

* View restaurant
* View categories
* View menu items
* View item descriptions
* View prices
* Select modifiers
* Add items to cart
* Add item notes
* View cart
* Place order
* View order status
* Request waiter
* Request bill

Customer accounts are **not required for the MVP**.

---

# 5. Authentication

The MVP must support authenticated staff accounts.

## Roles

At minimum:

### Admin

Full access.

### Manager

Operational management and reporting.

### Steward

Ordering and table operations.

### Cashier

Billing and payments.

### Kitchen

Kitchen order interface.

### Bar

Bar order interface.

---

# 6. Permission System

Permissions should be defined independently from UI roles.

Example permissions:

```text
restaurant.view
restaurant.edit

tables.view
tables.create
tables.edit
tables.delete
tables.merge
tables.transfer

menu.view
menu.create
menu.edit
menu.delete
menu.change_price

orders.view
orders.create
orders.edit
orders.cancel
orders.void

discounts.view
discounts.apply
discounts.manage

payments.create
payments.refund

reports.view

printers.view
printers.manage

users.view
users.manage
```

The MVP should support role-based permission assignment.

---

# 7. Restaurant Configuration

Admin must be able to configure:

* Restaurant name
* Logo
* Address
* Phone number
* Currency
* Tax settings
* Service charge
* Receipt footer
* Business hours
* Time zone

Example:

```text
Restaurant:
Teya Beach

Currency:
USD

Tax:
10%

Service Charge:
5%
```

---

# 8. Floor and Table Management

Admin must be able to create restaurant floor layouts.

## Floor

Properties:

```text
id
name
display_order
```

Example:

```text
Main Floor
Outdoor
Bar
Upper Floor
```

## Table

Properties:

```text
id
name
capacity
floor
position_x
position_y
width
height
rotation
status
qr_token
```

### Table operations

Admin can:

* Create table
* Rename table
* Delete table
* Change capacity
* Move table
* Resize table
* Rotate table
* Assign floor
* Generate QR code
* Disable table

---

# 9. Table States

The MVP should support:

```text
AVAILABLE
OCCUPIED
ORDERING
READY
BILL_REQUESTED
PAYMENT_PENDING
CLOSED
OUT_OF_SERVICE
```

The UI should provide clear visual indication of table status.

---

# 10. QR Codes

Each table must have a unique QR token.

Example:

```text
Table 07

QR:
████████████
████████████
████████████
```

The QR URL must identify the table without exposing internal database IDs.

Example:

```text
/order/7f4b9a2c...
```

Admin must be able to:

* Generate QR
* Regenerate QR
* Download/print QR
* Disable QR
* Associate QR with another table

---

# 11. Menu Management

Admin must be able to create menu categories.

Example:

```text
Food
 ├── Starters
 ├── Burgers
 ├── Pizza
 └── Rice

Drinks
 ├── Soft Drinks
 ├── Cocktails
 └── Beer

Desserts
```

---

# 12. Menu Items

Each menu item should support:

```text
id
name
description
category
price
image
tax
printer
available
display_order
```

Example:

```text
Chicken Burger

Description:
Grilled chicken, lettuce and house sauce.

Price:
$8.50

Printer:
Kitchen

Available:
YES
```

---

# 13. Menu Item Modifiers

Items must support optional modifiers.

Examples:

```text
Chicken Burger

Size:
Regular
Large +$2

Extras:
Cheese +$1
Bacon +$2
Extra Patty +$3
```

Modifier properties:

```text
name
price_adjustment
minimum_selection
maximum_selection
required
```

---

# 14. Item Availability

Admin/staff with permission must be able to mark items:

```text
AVAILABLE
UNAVAILABLE
```

An unavailable item must:

* Stop appearing as orderable to customers
* Remain visible to admins
* Remain visible in historical orders

---

# 15. Pricing

The pricing engine must calculate:

```text
Item Price
+ Modifier Prices
- Discounts
+ Service Charge
+ Tax
= Grand Total
```

Prices must be stored with orders when the order is placed.

Historical orders must **not change** when menu prices are changed later.

---

# 16. Discounts

MVP discount types:

### Percentage

```text
10% OFF
```

### Fixed amount

```text
$5 OFF
```

### Item discount

```text
Burger - 10%
```

### Category discount

```text
Drinks - 20%
```

Discounts should support:

```text
name
type
value
start_date
end_date
active
```

Optional time restrictions may be added for happy-hour discounts.

---

# 17. Orders

An order must contain:

```text
id
table_id
created_by
source
status
subtotal
discount
tax
service_charge
total
created_at
updated_at
```

## Order sources

```text
STEWARD
CUSTOMER_QR
ADMIN
```

---

# 18. Order Items

Each order item must contain a historical snapshot of:

```text
menu item
item name
unit price
quantity
modifiers
notes
discount
subtotal
```

This ensures historical orders remain accurate even if menu data changes.

---

# 19. Order Notes

Customers and stewards can add notes.

Examples:

```text
No onions
Extra spicy
No ice
Allergy note
```

Notes must appear on the relevant KOT/BOT.

---

# 20. Order State Machine

Orders should follow a controlled lifecycle.

```text
DRAFT
  ↓
PLACED
  ↓
ACCEPTED
  ↓
PREPARING
  ↓
READY
  ↓
SERVED
  ↓
BILLED
  ↓
PAID
  ↓
CLOSED
```

Additional states:

```text
CANCELLED
VOIDED
REFUNDED
```

State transitions must be validated by the backend.

---

# 21. Customer Ordering

Customer flow:

```text
Scan QR
   ↓
Menu
   ↓
Select category
   ↓
Select item
   ↓
Select modifiers
   ↓
Add to cart
   ↓
Review order
   ↓
Place order
```

After placing the order:

```text
Order #1042

✓ Order received

Kitchen:
Preparing

Bar:
Preparing
```

Customer should be able to view the status of their active order.

---

# 22. Steward Ordering

Steward flow:

```text
Login
 ↓
Floor
 ↓
Select table
 ↓
Create order
 ↓
Add items
 ↓
Add notes/modifiers
 ↓
Send order
```

Stewards should be able to add additional items to an existing order.

Example:

```text
Order #1042

Existing:
2 × Burger

Additional:
1 × Fries
2 × Coke
```

Only the newly added items should be sent as a new KOT/BOT where appropriate.

---

# 23. Kitchen / Bar Routing

Every menu item should be assignable to a printer/station.

Example:

```text
Chicken Burger → Kitchen
Fries          → Kitchen
Beer           → Bar
Mojito         → Bar
```

An order containing:

```text
2 × Burger
1 × Fries
2 × Beer
```

must generate:

### Kitchen KOT

```text
TABLE 07

2 × Chicken Burger
1 × Fries
```

### Bar BOT

```text
TABLE 07

2 × Beer
```

---

# 24. Thermal Printer Support

MVP must support network-connected thermal printers.

Printer configuration:

```text
name
ip_address
port
type
station
paper_width
active
```

Example:

```text
Kitchen Printer
192.168.1.50
Port: 9100
Station: KITCHEN

Bar Printer
192.168.1.51
Port: 9100
Station: BAR

Receipt Printer
192.168.1.52
Port: 9100
Station: RECEIPT
```

---

# 25. Printing

The system must support:

### KOT

Kitchen Order Ticket.

### BOT

Bar Order Ticket.

### Receipt

Customer payment receipt.

### Bill

Pre-payment bill.

### Reprint

Authorized users can reprint:

* KOT
* BOT
* Bill
* Receipt

Reprints must be marked as reprints.

---

# 26. Printer Failure Handling

The server must detect printing failures where possible.

Example:

```text
⚠ Kitchen Printer Offline

Order #1042

[Retry]
[Print on Backup Printer]
```

The system should maintain a print-job queue.

Example:

```text
PRINT_PENDING
PRINTING
PRINTED
FAILED
```

Failed jobs should be retryable.

---

# 27. Kitchen Display

The MVP should include a basic Kitchen Display System.

Orders appear as cards:

```text
┌────────────────────────────┐
│ TABLE 07                   │
│ ORDER #1042                │
│                            │
│ 2 × Chicken Burger         │
│ 1 × Fries                  │
│                            │
│ 12:42 PM                   │
│                            │
│ [ ACCEPT ]                 │
└────────────────────────────┘
```

Order states:

```text
NEW
ACCEPTED
PREPARING
READY
```

---

# 28. Bar Display

The same system should support a separate Bar display.

Bar staff only see items routed to the Bar station.

Example:

```text
BAR

TABLE 07

2 × Beer
1 × Mojito

[START]
[READY]
```

---

# 29. Real-Time Communication

The server should use WebSockets for real-time events.

Examples:

```text
Customer places order
        ↓
Steward receives notification

Kitchen marks order READY
        ↓
Steward receives notification

Customer requests bill
        ↓
Steward receives notification
```

Events should include:

```text
ORDER_CREATED
ORDER_UPDATED
ORDER_ACCEPTED
ORDER_READY
ORDER_CANCELLED
TABLE_UPDATED
PAYMENT_CREATED
CUSTOMER_REQUEST
PRINTER_STATUS_CHANGED
```

---

# 30. Customer Requests

MVP customer requests:

```text
CALL_WAITER
REQUEST_BILL
REQUEST_ASSISTANCE
```

Example:

```text
TABLE 07

🔔 Customer requested assistance
```

Steward can mark the request as:

```text
OPEN
ACKNOWLEDGED
COMPLETED
```

---

# 31. Billing

The billing system must support:

* Generate bill
* Apply discount
* Tax
* Service charge
* Print bill
* Record payment
* Print receipt
* Reprint receipt

---

# 32. Payment Methods

MVP:

```text
CASH
CARD
BANK_TRANSFER
OTHER
```

Payment record:

```text
id
order_id
method
amount
reference
received_by
created_at
```

---

# 33. Split Payments

The MVP should support splitting a bill.

Example:

```text
Total: $100

Cash: $40
Card: $60
```

The order becomes fully paid only when:

```text
Total Paid >= Total Due
```

---

# 34. Table Transfer

Authorized staff can transfer an active table.

Example:

```text
Table 07
   ↓
Transfer
   ↓
Table 12
```

The order remains unchanged.

---

# 35. Table Merge

Authorized staff can merge tables.

Example:

```text
Table 07
+
Table 08
      ↓
Merged Table
```

Orders should remain traceable to their original source tables.

---

# 36. Audit Log

Important actions must be recorded.

Examples:

```text
User changed menu price
User applied discount
User voided item
User cancelled order
User refunded payment
User transferred table
User reprinted receipt
User changed restaurant settings
```

Audit record:

```text
id
user_id
action
entity_type
entity_id
old_value
new_value
timestamp
```

Audit logs should not be editable through the normal UI.

---

# 37. Dashboard

Admin dashboard should show:

```text
TODAY

Revenue
Orders
Average Order Value
Paid Orders
Open Tables
Active Orders
```

Example:

```text
Revenue          $4,820
Orders           183
Avg Order        $26.34
Open Tables      12
```

---

# 38. Basic Analytics

MVP reports:

## Sales

* Daily sales
* Weekly sales
* Monthly sales
* Sales by category
* Sales by menu item

## Orders

* Total orders
* Average order value
* Orders by source
* Orders by hour

## Payments

* Cash
* Card
* Bank transfer
* Other

## Discounts

* Discount total
* Discount count
* Discount by type

## Staff

* Orders created by steward
* Sales associated with steward

---

# 39. Reporting

Reports should support date ranges:

```text
Today
Yesterday
This Week
This Month
Custom Range
```

Reports should be viewable inside the application.

CSV export should be supported for important reports.

---

# 40. Currency

The MVP should support configurable currency.

Example:

```text
USD
LKR
EUR
GBP
```

Currency must be stored at the restaurant configuration level.

Monetary values should **not use floating-point types**.

Use integer minor units or a decimal representation.

Example:

```text
$10.50
→ 1050 cents
```

---

# 41. Database

Recommended MVP database:

**SQLite**

The database should contain at minimum:

```text
users
roles
permissions
role_permissions

restaurant_settings

floors
tables
table_qr_tokens

menu_categories
menu_items
menu_modifiers
menu_item_modifiers

orders
order_items
order_item_modifiers

discounts
discount_applications

payments

printers
print_jobs

customer_requests

audit_logs
```

---

# 42. Data Integrity

The server must enforce business rules.

Clients must never be trusted to calculate:

* Prices
* Discounts
* Taxes
* Totals
* Permissions
* Order state transitions

The server is authoritative.

---

# 43. Offline / LAN Operation

The restaurant must continue operating without external Internet access.

Required:

```text
LAN
 ↓
Rust Server
 ↓
SQLite
```

Internet should not be required for:

* Admin
* Steward ordering
* Customer QR ordering
* Kitchen
* Bar
* Printing
* Billing
* Analytics

---

# 44. Network Discovery

The MVP should provide an easy way for clients to locate the restaurant server.

Possible approaches:

```text
restaurant.local
```

or mDNS/service discovery.

Fallback:

```text
192.168.x.x
```

Admin should be able to see the server's current LAN address.

---

# 45. Security

Minimum requirements:

* Password hashing
* Session/token authentication
* Role-based permissions
* HTTPS where practical
* Input validation
* SQL parameterization
* CSRF protection where applicable
* Rate limiting for customer endpoints
* Random QR tokens
* No exposure of internal database IDs through customer URLs
* Audit logging

Customer QR tokens should be sufficiently random and unguessable.

---

# 46. Backup

The MVP should provide local database backup.

Admin:

```text
Settings
 ↓
Backup
 ↓
Create Backup
```

Backup should contain:

* SQLite database
* Restaurant configuration
* Menu
* Tables
* Orders
* Payments
* Audit logs

Automatic scheduled backups can be added after the MVP.

---

# 47. System Health

Admin should have a basic system-health screen.

Example:

```text
SERVER        ✓ ONLINE
DATABASE      ✓ OK

KITCHEN       ✓ ONLINE
BAR           ✓ ONLINE
RECEIPT       ⚠ OFFLINE

CONNECTED DEVICES
Stewards      3
Kitchen       1
Customers     8
```

---

# 48. Error Handling

Errors must be user-friendly.

Bad:

```text
SQLITE_CONSTRAINT_UNIQUE
```

Good:

```text
Unable to create table.

A table with this name already exists.
```

Technical details should be available in server logs.

---

# 49. Logging

Rust server should use structured logging.

Recommended levels:

```text
TRACE
DEBUG
INFO
WARN
ERROR
```

Important events:

```text
Server started
Database connected
User logged in
Order created
Order updated
Print job created
Print failed
Printer disconnected
Payment created
Unexpected error
```

---

# 50. MVP Non-Goals

The following should **not** be required for MVP:

* Cloud synchronization
* Multi-branch support
* Advanced inventory
* Recipe management
* Supplier management
* Purchase orders
* Loyalty program
* Customer accounts
* Online delivery
* Third-party delivery integrations
* Accounting integrations
* Advanced CRM
* AI recommendations
* Advanced reservations
* Payroll
* Employee attendance
* Multi-currency orders
* Cryptocurrency payments

These can be added later.

---

# 51. Suggested Technology Stack

## Backend

```text
Rust
Tokio
Axum
SQLx
SQLite
Serde
Tracing
Tower
WebSockets
```

## Desktop

Preferred:

```text
Tauri
+
Svelte / React
```

Alternative:

```text
Slint
+
Rust
```

## Web

Recommended:

```text
SvelteKit
```

or:

```text
React
```

The customer and steward applications can share UI components where appropriate.

---

# 52. Suggested Rust Workspace

```text
restaurant-system/
│
├── Cargo.toml
│
├── apps/
│   ├── server/
│   └── desktop/
│
├── crates/
│   ├── domain/
│   ├── database/
│   ├── api/
│   ├── auth/
│   ├── orders/
│   ├── menu/
│   ├── tables/
│   ├── pricing/
│   ├── payments/
│   ├── printing/
│   ├── analytics/
│   └── audit/
│
└── web/
    ├── steward/
    ├── customer/
    └── kitchen/
```

---

# 53. MVP Definition of Done

The MVP is considered complete when a restaurant can:

### Setup

* [ ] Install server
* [ ] Create administrator
* [ ] Configure restaurant
* [ ] Create floors
* [ ] Create tables
* [ ] Generate table QR codes
* [ ] Create menu categories
* [ ] Create menu items
* [ ] Configure modifiers
* [ ] Configure printers
* [ ] Create staff users

### Customer

* [ ] Scan QR
* [ ] Open menu
* [ ] Select items
* [ ] Select modifiers
* [ ] Add notes
* [ ] Place order
* [ ] See order status
* [ ] Request waiter
* [ ] Request bill

### Steward

* [ ] Login
* [ ] View tables
* [ ] Create orders
* [ ] Modify orders
* [ ] Send KOT/BOT
* [ ] See kitchen/bar status
* [ ] Transfer table
* [ ] Merge tables
* [ ] Request bill
* [ ] Process payment
* [ ] Print receipt

### Kitchen / Bar

* [ ] Receive orders
* [ ] View order items
* [ ] Accept orders
* [ ] Mark preparing
* [ ] Mark ready
* [ ] Print KOT/BOT

### Billing

* [ ] Generate bill
* [ ] Apply discount
* [ ] Calculate tax
* [ ] Calculate service charge
* [ ] Accept payment
* [ ] Support multiple payment methods
* [ ] Support split payment
* [ ] Print receipt
* [ ] Reprint receipt

### Admin

* [ ] Manage menu
* [ ] Manage pricing
* [ ] Manage tables
* [ ] Manage QR codes
* [ ] Manage printers
* [ ] Manage users
* [ ] Manage roles
* [ ] View dashboard
* [ ] View sales reports
* [ ] Export reports
* [ ] View audit logs
* [ ] Create database backup

### Reliability

* [ ] Operates without Internet
* [ ] Recovers cleanly after server restart
* [ ] Does not lose completed orders
* [ ] Handles printer failures
* [ ] Supports print retry
* [ ] Maintains audit history
* [ ] Maintains historical prices on completed orders

---

# 54. Future Roadmap

After MVP:

## Phase 2

```text
Inventory
Recipes
Stock management
Purchase orders
Suppliers
Low-stock alerts
```

## Phase 3

```text
Cloud backup
Remote admin
Multi-location
Centralized analytics
```

## Phase 4

```text
Customer accounts
Loyalty
Promotions
Reservations
Online ordering
Delivery
```

## Phase 5

```text
Accounting
Payroll
Staff attendance
Advanced business intelligence
AI forecasting
```

---

# 55. Core Design Principle

The most important architectural principle is:

> **The restaurant must remain operational even when the Internet is unavailable.**

The local Rust server should therefore be the **source of truth**.

```text
                    INTERNET
                       │
                 OPTIONAL
                       │
                       ▼
              ┌────────────────┐
              │ Cloud Services │
              └───────┬────────┘
                      │
                      │
              ┌───────▼────────┐
              │  LOCAL SERVER  │
              │                │
              │  SOURCE OF     │
              │     TRUTH      │
              └───────┬────────┘
                      │
        ┌─────────────┼──────────────┐
        │             │              │
        ▼             ▼              ▼
      ADMIN        STEWARD       CUSTOMER
        │             │              │
        └─────────────┼──────────────┘
                      │
              ┌───────▼────────┐
              │ Kitchen / Bar  │
              │   Printers     │
              └────────────────┘
```

The MVP should prioritize **reliability, speed, simple workflows, and data integrity** over feature count.
