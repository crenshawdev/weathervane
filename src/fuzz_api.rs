// SPDX-License-Identifier: MIT OR Apache-2.0

//! Parser entry points for the cargo-fuzz targets in `fuzz/`. Built only with
//! the `fuzzing` feature and not part of the public API.

pub use crate::air_quality::fuzz::*;
pub use crate::air_quality_aqicn::fuzz::*;
pub use crate::alerts::fuzz::*;
pub use crate::geo::fuzz::*;
pub use crate::location::fuzz::*;
pub use crate::pollen::fuzz::*;
pub use crate::weather::fuzz::*;
pub use crate::weather_jma::fuzz::*;
pub use crate::weather_nws::fuzz::*;
