#![no_std]
#![deny(unsafe_code)]
#![warn(clippy::all, clippy::pedantic)]

pub mod frame;
pub mod mavlink_compat;

pub mod crypto {
    pub mod aes_gcm;
    pub mod ecdh;
    pub mod hkdf;
    pub mod hmac;
    pub mod nonce;
}

pub mod channel {
    pub mod manager;
    pub mod voter;
    pub mod failover;
}

pub mod anti_jam {
    pub mod detector;
    pub mod fhss;
    pub mod dsss;
}

pub use frame::{JflFrame, JflError};
