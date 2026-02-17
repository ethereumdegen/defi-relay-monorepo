//! Actix-web middleware for enforcing [x402](https://www.x402.org) payments on protected routes.
//!
//! This middleware validates incoming payment headers using a configured x402 facilitator,
//! and settles valid payments either before or after request execution (configurable).
//!
//! Returns a `402 Payment Required` response if the request lacks a valid payment.
//!
//! ## Example Usage
//!
//! ```rust,ignore
//! use alloy_primitives::address;
//! use actix_web::{web, App, HttpServer, HttpResponse};
//! use x402_actix::X402Middleware;
//! use x402_rs::networks::{KnownNetworkEip155, USDC};
//! use x402_rs::scheme::v1_eip155_exact::V1Eip155Exact;
//!
//! let x402 = X402Middleware::new("https://facilitator.x402.rs");
//!
//! App::new().service(
//!     web::resource("/protected")
//!         .wrap(x402.with_price_tag(V1Eip155Exact::price_tag(
//!             address!("0xd8dA6BF26964aF9D7eEd9e03E53415D37aA96045"),
//!             USDC::base_sepolia().parse("0.01").unwrap(),
//!         )))
//!         .route(web::get().to(|| async { HttpResponse::Ok().body("VIP content") }))
//! );
//! ```
//!
//! See [`X402Middleware`] for full configuration options.
//! For low-level interaction with the facilitator, see [`facilitator_client::FacilitatorClient`].

pub mod facilitator_client;
pub mod middleware;
pub mod paygate;

pub use middleware::{X402LayerBuilder, X402Middleware};
pub use paygate::{DynamicPriceTags, PaygateProtocol, PriceTagSource, StaticPriceTags};
