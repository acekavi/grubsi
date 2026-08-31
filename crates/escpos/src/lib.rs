//! ESC/POS ticket rendering and printer transports.
//!
//! M0 provides the transport layer and its test double. Rendering
//! (`Document`, `render`, `encode`) arrives in M4 with the print queue.

pub mod sink;
pub mod transport;
