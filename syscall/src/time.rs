//! see <https://github.com/torvalds/linux/blob/master/include/uapi/linux/time.h>.

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
#[repr(transparent)]
pub struct ClockId(pub usize);

impl ClockId {
    pub const CLOCK_REALTIME: Self = Self(0);
    pub const CLOCK_MONOTONIC: Self = Self(1);
    pub const CLOCK_PROCESS_CPUTIME_ID: Self = Self(2);
    pub const CLOCK_THREAD_CPUTIME_ID: Self = Self(3);
    pub const CLOCK_MONOTONIC_RAW: Self = Self(4);
    pub const CLOCK_REALTIME_COARSE: Self = Self(5);
    pub const CLOCK_MONOTONIC_COARSE: Self = Self(6);
    pub const CLOCK_BOOTTIME: Self = Self(7);
    pub const CLOCK_REALTIME_ALARM: Self = Self(8);
    pub const CLOCK_BOOTTIME_ALARM: Self = Self(9);
    pub const CLOCK_SGI_CYCLE: Self = Self(10);
    pub const CLOCK_TAI: Self = Self(11);
}

#[derive(Clone, Copy, PartialEq, PartialOrd, Eq, Ord, Debug)]
#[repr(C)]
pub struct TimeSpec {
    // seconds
    pub tv_sec: usize,
    // nanoseconds
    pub tv_nsec: usize,
}

impl TimeSpec {
    pub const ZERO: Self = Self {
        tv_sec: 0,
        tv_nsec: 0,
    };
    pub const SECOND: Self = Self {
        tv_sec: 1,
        tv_nsec: 0,
    };
    pub const MILLSECOND: Self = Self {
        tv_sec: 0,
        tv_nsec: 1_000_000,
    };
    pub const MICROSECOND: Self = Self {
        tv_sec: 0,
        tv_nsec: 1_000,
    };
    pub const NANOSECOND: Self = Self {
        tv_sec: 0,
        tv_nsec: 1,
    };
    pub fn from_millsecond(millsecond: usize) -> Self {
        Self {
            tv_sec: millsecond / 1_000,
            tv_nsec: millsecond % 1_000 * 1_000_000,
        }
    }
}

impl core::ops::Add<TimeSpec> for TimeSpec {
    type Output = Self;
    fn add(self, rhs: Self) -> Self::Output {
        let mut ans = Self {
            tv_sec: self.tv_sec + rhs.tv_sec,
            tv_nsec: self.tv_nsec + rhs.tv_nsec,
        };
        if ans.tv_nsec > 1_000_000_000 {
            ans.tv_sec += 1;
            ans.tv_nsec -= 1_000_000_000;
        }
        ans
    }
}

impl core::fmt::Display for TimeSpec {
    fn fmt(&self, f: &mut core::fmt::Formatter) -> core::fmt::Result {
        write!(f, "TimeSpec({}.{:09})", self.tv_sec, self.tv_nsec)
    }
}
