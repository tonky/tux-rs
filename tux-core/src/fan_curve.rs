use serde::{Deserialize, Serialize};

/// A single point on a fan curve: temperature → fan speed.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FanCurvePoint {
    /// Temperature threshold in °C.
    pub temp: u8,
    /// Fan speed as a percentage (0–100).
    pub speed: u8,
}

/// Fan operating mode.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum FanMode {
    /// Restore hardware/firmware automatic fan control.
    Auto,
    /// User-set static PWM — daemon does not intervene.
    Manual,
    /// Poll temperature → interpolate curve → write PWM.
    CustomCurve,
}

/// Configuration for the fan curve engine.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct FanConfig {
    pub mode: FanMode,
    /// Minimum fan speed in percent (applied after interpolation).
    pub min_speed_percent: u8,
    /// Fan curve points (sorted by temperature ascending).
    pub curve: Vec<FanCurvePoint>,
    /// Poll interval in milliseconds when temperature is changing.
    pub active_poll_ms: u64,
    /// Poll interval in milliseconds when temperature is stable.
    pub idle_poll_ms: u64,
    /// Hysteresis in °C — skip update if temp change is smaller.
    pub hysteresis_degrees: u8,
}

impl Default for FanConfig {
    fn default() -> Self {
        Self {
            mode: FanMode::CustomCurve,
            min_speed_percent: 25,
            curve: vec![
                FanCurvePoint { temp: 0, speed: 0 },
                FanCurvePoint {
                    temp: 25,
                    speed: 10,
                },
                FanCurvePoint {
                    temp: 50,
                    speed: 30,
                },
                FanCurvePoint {
                    temp: 75,
                    speed: 70,
                },
                FanCurvePoint {
                    temp: 100,
                    speed: 100,
                },
            ],
            active_poll_ms: 2000,
            idle_poll_ms: 1000,
            hysteresis_degrees: 3,
        }
    }
}

/// Minimum poll interval to prevent busy-looping.
const MIN_POLL_MS: u64 = 10;

impl FanConfig {
    /// Validate the configuration, returning an error message if invalid.
    pub fn validate(&self) -> Result<(), &'static str> {
        if self.active_poll_ms < MIN_POLL_MS {
            return Err("active_poll_ms too low (min 10)");
        }
        if self.idle_poll_ms < MIN_POLL_MS {
            return Err("idle_poll_ms too low (min 10)");
        }
        if self.min_speed_percent > 100 {
            return Err("min_speed_percent must be 0–100");
        }
        for p in &self.curve {
            if p.speed > 100 {
                return Err("curve speed must be 0–100");
            }
        }
        // Check curve is sorted by temperature.
        for pair in self.curve.windows(2) {
            if pair[1].temp < pair[0].temp {
                return Err("curve must be sorted by temperature ascending");
            }
        }
        Ok(())
    }
}

/// Linear interpolation between curve points.
///
/// - Below the first point → first point's speed.
/// - Above the last point  → last point's speed.
/// - Empty curve → 100 (safety fallback).
/// - Points are assumed sorted by `temp` ascending.
pub fn interpolate(curve: &[FanCurvePoint], temp: u8) -> u8 {
    if curve.is_empty() {
        return SAFETY_FALLBACK_SPEED;
    }

    if temp <= curve[0].temp {
        return curve[0].speed;
    }

    let last = &curve[curve.len() - 1];
    if temp >= last.temp {
        return last.speed;
    }

    // Find the segment containing `temp`.
    for pair in curve.windows(2) {
        let lo = &pair[0];
        let hi = &pair[1];
        let t_lo = lo.temp as i32;
        let t_hi = hi.temp as i32;
        let s_lo = lo.speed as i32;
        let s_hi = hi.speed as i32;
        let temp = temp as i32;

        if temp >= t_lo && temp <= t_hi {
            if t_hi == t_lo {
                return hi.speed;
            }
            // Linear interpolation using signed math to handle descending curves.
            let result = s_lo + (s_hi - s_lo) * (temp - t_lo) / (t_hi - t_lo);
            return result.clamp(0, SAFETY_FALLBACK_SPEED as i32) as u8;
        }
    }

    // Should not reach here if curve is sorted, but safety fallback.
    SAFETY_FALLBACK_SPEED
}

/// Standard PWM maximum value.
const PWM_MAX: u16 = 255;
/// Rounding offset for percent-to-PWM conversion (PERCENT_MAX / 2).
const PWM_ROUNDING: u16 = 50;
/// Percentage divisor.
const PERCENT_MAX: u16 = 100;
/// Safety fallback speed (100%) when curve is empty or beyond range.
const SAFETY_FALLBACK_SPEED: u8 = 100;

/// Convert a percentage (0–100) to a PWM value (0–255).
pub fn percent_to_pwm(percent: u8) -> u8 {
    let percent = percent.min(PERCENT_MAX as u8) as u16;
    ((percent * PWM_MAX + PWM_ROUNDING) / PERCENT_MAX) as u8
}

/// Uniwill EC PWM scale maximum.
const EC_PWM_MAX: u16 = 200;

/// Convert a fan curve + min_speed into EC fan table zones.
///
/// Samples the interpolated curve at regular temperature intervals to
/// generate up to 16 `(end_temp, ec_speed)` zones suitable for
/// programming the Uniwill EC's native fan table.
pub fn curve_to_ec_zones(curve: &[FanCurvePoint], min_speed_percent: u8) -> Vec<(u8, u8)> {
    if curve.is_empty() {
        // Safety: one zone spanning 0–115°C at max speed.
        return vec![(115, EC_PWM_MAX as u8)];
    }

    // Sample at ~7°C intervals: 0..112 step 7 = 16 zones.
    // Zone boundaries: 7, 14, 21, ..., 105, 112, then cap last at 115.
    let n_zones: u8 = 16;
    let step: u8 = 7;
    let mut zones = Vec::with_capacity(n_zones as usize);

    for i in 0..n_zones {
        let end_temp = if i == n_zones - 1 {
            115 // last zone covers up to 115°C
        } else {
            (i + 1) * step
        };

        // Sample curve at zone midpoint for a good approximation.
        let start_temp = i * step;
        let mid_temp = start_temp + (end_temp - start_temp) / 2;
        let speed_pct = interpolate(curve, mid_temp).max(min_speed_percent);

        // Convert percent (0–100) → EC scale (0–200).
        let ec_speed = ((speed_pct as u16 * EC_PWM_MAX + 50) / 100) as u8;
        // Clamp to EC minimum (the kernel enforces 20 minimum, but be safe).
        let ec_speed = ec_speed.max(20);

        zones.push((end_temp, ec_speed));
    }

    zones
}

#[cfg(test)]
mod tests {
    use super::*;

    fn default_curve() -> Vec<FanCurvePoint> {
        vec![
            FanCurvePoint { temp: 0, speed: 0 },
            FanCurvePoint {
                temp: 25,
                speed: 10,
            },
            FanCurvePoint {
                temp: 50,
                speed: 30,
            },
            FanCurvePoint {
                temp: 75,
                speed: 70,
            },
            FanCurvePoint {
                temp: 100,
                speed: 100,
            },
        ]
    }

    #[test]
    fn interpolate_below_first_point() {
        let curve = default_curve();
        assert_eq!(interpolate(&curve, 0), 0);
    }

    #[test]
    fn interpolate_above_last_point() {
        let curve = default_curve();
        assert_eq!(interpolate(&curve, 100), 100);
        assert_eq!(interpolate(&curve, 255), 100);
    }

    #[test]
    fn interpolate_exact_match() {
        let curve = default_curve();
        assert_eq!(interpolate(&curve, 0), 0);
        assert_eq!(interpolate(&curve, 25), 10);
        assert_eq!(interpolate(&curve, 50), 30);
        assert_eq!(interpolate(&curve, 75), 70);
        assert_eq!(interpolate(&curve, 100), 100);
    }

    #[test]
    fn interpolate_between_points() {
        let curve = default_curve();
        // 12°C between 0→0% and 25→10%: 0 + (10-0)*(12/25) = 4%
        assert_eq!(interpolate(&curve, 12), 4);
        // 37°C between 25→10% and 50→30%: 10 + (30-10)*(12/25) = 10 + 9 = 19%
        assert_eq!(interpolate(&curve, 37), 19);
    }

    #[test]
    fn interpolate_empty_curve() {
        assert_eq!(interpolate(&[], 50), 100);
    }

    #[test]
    fn interpolate_descending_curve() {
        // Curve with a descending segment: 60→80% then 80→30%
        let curve = vec![
            FanCurvePoint {
                temp: 60,
                speed: 80,
            },
            FanCurvePoint {
                temp: 80,
                speed: 30,
            },
        ];
        assert_eq!(interpolate(&curve, 60), 80);
        assert_eq!(interpolate(&curve, 80), 30);
        // 70°C midpoint: 80 + (30-80)*(10/20) = 80 - 25 = 55
        assert_eq!(interpolate(&curve, 70), 55);
    }

    #[test]
    fn interpolate_single_point() {
        let curve = vec![FanCurvePoint {
            temp: 60,
            speed: 50,
        }];
        assert_eq!(interpolate(&curve, 30), 50);
        assert_eq!(interpolate(&curve, 60), 50);
        assert_eq!(interpolate(&curve, 90), 50);
    }

    #[test]
    fn percent_to_pwm_boundaries() {
        assert_eq!(percent_to_pwm(0), 0);
        assert_eq!(percent_to_pwm(100), 255);
    }

    #[test]
    fn percent_to_pwm_midpoint() {
        // (50 * 255 + 50) / 100 = 12800 / 100 = 128
        assert_eq!(percent_to_pwm(50), 128);
    }

    #[test]
    fn percent_to_pwm_clamps_above_100() {
        assert_eq!(percent_to_pwm(200), 255);
    }

    #[test]
    fn default_config_is_valid() {
        assert!(FanConfig::default().validate().is_ok());
    }

    #[test]
    fn validate_rejects_low_poll_ms() {
        let cfg = FanConfig {
            active_poll_ms: 0,
            ..Default::default()
        };
        assert!(cfg.validate().is_err());
    }

    #[test]
    fn validate_rejects_speed_above_100() {
        let mut cfg = FanConfig::default();
        cfg.curve.push(FanCurvePoint {
            temp: 95,
            speed: 120,
        });
        assert!(cfg.validate().is_err());
    }

    #[test]
    fn validate_rejects_unsorted_curve() {
        let cfg = FanConfig {
            curve: vec![
                FanCurvePoint {
                    temp: 80,
                    speed: 80,
                },
                FanCurvePoint {
                    temp: 60,
                    speed: 30,
                },
            ],
            ..Default::default()
        };
        assert!(cfg.validate().is_err());
    }

    #[test]
    fn validate_rejects_min_speed_above_100() {
        let cfg = FanConfig {
            min_speed_percent: 150,
            ..Default::default()
        };
        assert!(cfg.validate().is_err());
    }

    #[test]
    fn curve_to_ec_zones_empty_curve_returns_safety() {
        let zones = curve_to_ec_zones(&[], 0);
        assert_eq!(zones.len(), 1);
        assert_eq!(zones[0], (115, 200));
    }

    #[test]
    fn curve_to_ec_zones_produces_16_zones() {
        let curve = default_curve();
        let zones = curve_to_ec_zones(&curve, 25);
        assert_eq!(zones.len(), 16);
        // Last zone should end at 115°C.
        assert_eq!(zones[15].0, 115);
    }

    #[test]
    fn curve_to_ec_zones_respects_min_speed() {
        let curve = vec![
            FanCurvePoint { temp: 0, speed: 0 },
            FanCurvePoint {
                temp: 100,
                speed: 50,
            },
        ];
        let zones = curve_to_ec_zones(&curve, 30);
        // All zone speeds should be at least 30% → 60 EC scale.
        for &(_, speed) in &zones {
            assert!(speed >= 60, "speed {speed} below min_speed 30% (60 EC)");
        }
    }

    #[test]
    fn curve_to_ec_zones_monotonically_increasing() {
        let curve = default_curve();
        let zones = curve_to_ec_zones(&curve, 0);
        for pair in zones.windows(2) {
            assert!(
                pair[1].1 >= pair[0].1,
                "zone speeds should be non-decreasing: {} < {}",
                pair[1].1,
                pair[0].1
            );
        }
    }

    #[test]
    fn curve_to_ec_zones_speed_range() {
        let curve = default_curve();
        let zones = curve_to_ec_zones(&curve, 0);
        for &(_, speed) in &zones {
            // EC minimum is 20, maximum is 200.
            assert!(speed >= 20, "speed {speed} below EC minimum 20");
            assert!(speed <= 200, "speed {speed} above EC maximum 200");
        }
    }
}
