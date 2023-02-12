use std::ops::{Add, AddAssign};
use std::time::Duration;

#[derive(Default)]
pub struct RuntimeTimings {
    /// The total runtime duration waiting on the producer.
    pub producer_wait_runtime: Duration,
    /// The total runtime duration waiting on the requests to execute.
    pub execute_wait_runtime: Duration,
}

impl Add for RuntimeTimings {
    type Output = Self;

    fn add(self, rhs: Self) -> Self::Output {
        Self {
            producer_wait_runtime: self.producer_wait_runtime
                + rhs.producer_wait_runtime,
            execute_wait_runtime: self.execute_wait_runtime + rhs.execute_wait_runtime,
        }
    }
}

impl AddAssign for RuntimeTimings {
    fn add_assign(&mut self, rhs: Self) {
        self.producer_wait_runtime += rhs.producer_wait_runtime;
        self.execute_wait_runtime += rhs.execute_wait_runtime;
    }
}

impl FromIterator<Self> for RuntimeTimings {
    fn from_iter<T: IntoIterator<Item = Self>>(iter: T) -> Self {
        let mut total = Self::default();
        for slf in iter {
            total += slf;
        }
        total
    }
}
