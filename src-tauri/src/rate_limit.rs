use std::collections::HashMap;
use std::net::IpAddr;
use std::time::{Duration, Instant};

#[derive(Debug, Clone)]
pub struct RateLimiter {
    max_attempts: u32,
    window: Duration,
    attempts: HashMap<IpAddr, Vec<Instant>>,
}

impl RateLimiter {
    pub fn new(max_attempts: u32, window_seconds: u64) -> Self {
        Self {
            max_attempts,
            window: Duration::from_secs(window_seconds),
            attempts: HashMap::new(),
        }
    }

    pub fn check_invalid_attempt(&mut self, ip: IpAddr) -> bool {
        let now = Instant::now();
        let attempts = self.attempts.entry(ip).or_default();
        attempts.retain(|attempt| now.duration_since(*attempt) <= self.window);
        if attempts.len() as u32 >= self.max_attempts {
            return false;
        }
        attempts.push(now);
        true
    }

    pub fn clear(&mut self, ip: IpAddr) {
        self.attempts.remove(&ip);
    }
}
