// Copyright (c) 2021 MASSA LABS <info@massa.net>

mod error;
pub use error::TimeError;
use std::fmt;
use std::time::{Duration, SystemTime, UNIX_EPOCH};
use std::{
    convert::{TryFrom, TryInto},
    str::FromStr,
};
use tokio::time::Instant;

use serde::{Deserialize, Serialize};

/// Time structure used every where.
/// Millis since 01/01/1970.
#[derive(Debug, Clone, Copy, Hash, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub struct UTime(u64);

impl fmt::Display for UTime {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.to_millis())
    }
}

impl From<u64> for UTime {
    /// Conversion from u64, representing timestamp in millis.
    /// ```
    /// # use time::*;
    /// let time : UTime = UTime::from(42);
    /// ```
    fn from(value: u64) -> Self {
        UTime(value)
    }
}

impl TryFrom<Duration> for UTime {
    type Error = TimeError;

    /// Conversion from `std::time::Duration`.
    /// ```
    /// # use std::time::Duration;
    /// # use time::*;
    /// # use std::convert::TryFrom;
    /// let duration: Duration = Duration::from_millis(42);
    /// let time : UTime = UTime::from(42);
    /// assert_eq!(time, UTime::try_from(duration).unwrap());
    /// ```
    fn try_from(value: Duration) -> Result<Self, Self::Error> {
        Ok(UTime(
            value
                .as_millis()
                .try_into()
                .map_err(|_| TimeError::ConversionError)?,
        ))
    }
}

impl From<UTime> for Duration {
    /// Conversion Utime to duration, representing timestamp in millis.
    /// ```
    /// # use std::time::Duration;
    /// # use time::*;
    /// # use std::convert::Into;
    /// let duration: Duration = Duration::from_millis(42);
    /// let time : UTime = UTime::from(42);
    /// let res: Duration = time.into();
    /// assert_eq!(res, duration);
    /// ```
    fn from(value: UTime) -> Self {
        Duration::from_millis(value.to_millis())
    }
}

impl FromStr for UTime {
    type Err = crate::TimeError;

    /// Conversion from `&str`.
    ///
    /// ```
    /// # use time::*;
    /// # use std::str::FromStr;
    /// let duration: &str = "42";
    /// let time : UTime = UTime::from(42);
    ///
    /// assert_eq!(time, UTime::from_str(duration).unwrap());
    /// ```
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Ok(UTime(
            u64::from_str(s).map_err(|_| Self::Err::ConversionError)?,
        ))
    }
}

impl UTime {
    /// Smallest time interval
    pub const EPSILON: UTime = UTime(1);

    /// Gets current timestamp.
    ///
    /// ```
    /// # use std::time::{Duration, SystemTime, UNIX_EPOCH};
    /// # use time::*;
    /// # use std::convert::TryFrom;
    /// # use std::cmp::max;
    /// let now_duration : Duration = SystemTime::now().duration_since(UNIX_EPOCH).unwrap();
    /// let now_utime : UTime = UTime::now(0).unwrap();
    /// let converted  :UTime = UTime::try_from(now_duration).unwrap();
    /// assert!(max(now_utime.saturating_sub(converted), converted.saturating_sub(now_utime)) < 100.into())
    /// ```
    pub fn now(compensation_millis: i64) -> Result<Self, TimeError> {
        let now: i64 = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map_err(|_| TimeError::TimeOverflowError)?
            .as_millis()
            .try_into()
            .map_err(|_| TimeError::TimeOverflowError)?;
        let compensated = now
            .checked_add(compensation_millis)
            .ok_or(TimeError::TimeOverflowError)?
            .try_into()
            .map_err(|_| TimeError::TimeOverflowError)?;
        Ok(UTime(compensated))
    }

    /// Conversion to `std::time::Duration`.
    /// ```
    /// # use std::time::Duration;
    /// # use time::*;
    /// let duration: Duration = Duration::from_millis(42);
    /// let time : UTime = UTime::from(42);
    /// let res: Duration = time.to_duration();
    /// assert_eq!(res, duration);
    /// ```
    pub fn to_duration(&self) -> Duration {
        Duration::from_millis(self.0)
    }

    /// Conversion to u64, representing millis.
    /// ```
    /// # use time::*;
    /// let time : UTime = UTime::from(42);
    /// let res: u64 = time.to_millis();
    /// assert_eq!(res, 42);
    /// ```
    pub fn to_millis(&self) -> u64 {
        self.0
    }

    /// ```
    /// # use std::time::{Duration, SystemTime, UNIX_EPOCH};
    /// # use time::*;
    /// # use std::convert::TryFrom;
    /// # use std::cmp::max;
    /// # use tokio::time::Instant;
    /// let (cur_timestamp, cur_instant): (UTime, Instant) = (UTime::now(0).unwrap(), Instant::now());
    /// let utime_instant: Instant = cur_timestamp.estimate_instant(0).unwrap();
    /// assert!(max(
    ///     utime_instant.saturating_duration_since(cur_instant),
    ///     cur_instant.saturating_duration_since(utime_instant)
    /// ) < std::time::Duration::from_millis(10))
    /// ```
    pub fn estimate_instant(self, compensation_millis: i64) -> Result<Instant, TimeError> {
        let (cur_timestamp, cur_instant): (UTime, Instant) =
            (UTime::now(compensation_millis)?, Instant::now());
        cur_instant
            .checked_add(self.to_duration())
            .ok_or(TimeError::TimeOverflowError)?
            .checked_sub(cur_timestamp.to_duration())
            .ok_or(TimeError::TimeOverflowError)
    }

    /// ```
    /// # use time::*;
    /// let time_1 : UTime = UTime::from(42);
    /// let time_2 : UTime = UTime::from(7);
    /// let res : UTime = time_1.saturating_sub(time_2);
    /// assert_eq!(res, UTime::from(42-7))
    /// ```
    pub fn saturating_sub(self, t: UTime) -> Self {
        UTime(self.0.saturating_sub(t.0))
    }

    /// ```
    /// # use time::*;
    /// let time_1 : UTime = UTime::from(42);
    /// let time_2 : UTime = UTime::from(7);
    /// let res : UTime = time_1.saturating_add(time_2);
    /// assert_eq!(res, UTime::from(42+7))
    /// ```
    pub fn saturating_add(self, t: UTime) -> Self {
        UTime(self.0.saturating_add(t.0))
    }

    /// ```
    /// # use time::*;
    /// let time_1 : UTime = UTime::from(42);
    /// let time_2 : UTime = UTime::from(7);
    /// let res : UTime = time_1.checked_sub(time_2).unwrap();
    /// assert_eq!(res, UTime::from(42-7))
    /// ```
    pub fn checked_sub(self, t: UTime) -> Result<Self, TimeError> {
        self.0
            .checked_sub(t.0)
            .ok_or_else(|| TimeError::CheckedOperationError("subtraction error".to_string()))
            .map(UTime)
    }

    /// ```
    /// # use time::*;
    /// let time_1 : UTime = UTime::from(42);
    /// let time_2 : UTime = UTime::from(7);
    /// let res : UTime = time_1.checked_add(time_2).unwrap();
    /// assert_eq!(res, UTime::from(42+7))
    /// ```
    pub fn checked_add(self, t: UTime) -> Result<Self, TimeError> {
        self.0
            .checked_add(t.0)
            .ok_or_else(|| TimeError::CheckedOperationError("addition error".to_string()))
            .map(UTime)
    }

    /// ```
    /// # use time::*;
    /// let time_1 : UTime = UTime::from(42);
    /// let time_2 : UTime = UTime::from(7);
    /// let res : u64 = time_1.checked_div_time(time_2).unwrap();
    /// assert_eq!(res,42/7)
    /// ```
    pub fn checked_div_time(self, t: UTime) -> Result<u64, TimeError> {
        self.0
            .checked_div(t.0)
            .ok_or_else(|| TimeError::CheckedOperationError("division error".to_string()))
    }

    /// ```
    /// # use time::*;
    /// let time_1 : UTime = UTime::from(42);
    /// let res : UTime = time_1.checked_div_u64(7).unwrap();
    /// assert_eq!(res,UTime::from(42/7))
    /// ```
    pub fn checked_div_u64(self, n: u64) -> Result<UTime, TimeError> {
        self.0
            .checked_div(n)
            .ok_or_else(|| TimeError::CheckedOperationError("division error".to_string()))
            .map(UTime)
    }

    /// ```
    /// # use time::*;
    /// let time_1 : UTime = UTime::from(42);
    /// let res : UTime = time_1.checked_mul(7).unwrap();
    /// assert_eq!(res,UTime::from(42*7))
    /// ```
    pub fn checked_mul(self, n: u64) -> Result<Self, TimeError> {
        self.0
            .checked_mul(n)
            .ok_or_else(|| TimeError::CheckedOperationError("multiplication error".to_string()))
            .map(UTime)
    }

    /// ```
    /// # use time::*;
    /// let time_1 : UTime = UTime::from(42);
    /// let time_2 : UTime = UTime::from(7);
    /// let res : UTime = time_1.checked_rem_time(time_2).unwrap();
    /// assert_eq!(res,UTime::from(42%7))
    /// ```
    pub fn checked_rem_time(self, t: UTime) -> Result<Self, TimeError> {
        self.0
            .checked_rem(t.0)
            .ok_or_else(|| TimeError::CheckedOperationError("remainder error".to_string()))
            .map(UTime)
    }

    /// ```
    /// # use time::*;
    /// let time_1 : UTime = UTime::from(42);
    /// let res : UTime = time_1.checked_rem_u64(7).unwrap();
    /// assert_eq!(res,UTime::from(42%7))
    /// ```
    pub fn checked_rem_u64(self, n: u64) -> Result<Self, TimeError> {
        self.0
            .checked_rem(n)
            .ok_or_else(|| TimeError::CheckedOperationError("remainder error".to_string()))
            .map(UTime)
    }
}
