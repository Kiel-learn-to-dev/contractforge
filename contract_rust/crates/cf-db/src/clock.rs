use chrono::{Local, NaiveDate, NaiveDateTime, Utc};

pub trait Clock: Send + Sync {
    fn today(&self) -> NaiveDate;
    fn now_utc(&self) -> NaiveDateTime;
    fn now_local(&self) -> NaiveDateTime;
}

#[derive(Debug, Default, Clone, Copy)]
pub struct SystemClock;

impl Clock for SystemClock {
    fn today(&self) -> NaiveDate {
        Local::now().date_naive()
    }
    fn now_utc(&self) -> NaiveDateTime {
        Utc::now().naive_utc()
    }
    fn now_local(&self) -> NaiveDateTime {
        Local::now().naive_local()
    }
}

#[derive(Debug, Clone, Copy)]
pub struct FixedClock {
    pub date: NaiveDate,
    pub utc: NaiveDateTime,
    pub local: NaiveDateTime,
}

impl Clock for FixedClock {
    fn today(&self) -> NaiveDate {
        self.date
    }
    fn now_utc(&self) -> NaiveDateTime {
        self.utc
    }
    fn now_local(&self) -> NaiveDateTime {
        self.local
    }
}
