#![no_std]
#![deny(unsafe_code)]
#![warn(clippy::all, clippy::pedantic)]

pub mod frame;
pub mod mavlink_compat;
pub mod native;

pub mod crypto {
    pub mod aes_gcm;
    pub mod ecdh;
    pub mod hkdf;
    pub mod hmac;
    pub mod nonce;
}

pub mod channel {
    pub mod failover;
    pub mod manager;
    pub mod voter;
}

pub mod anti_jam {
    pub mod detector;
    pub mod dsss;
    pub mod fhss;
}

pub use frame::{JflError, JflFrame};
