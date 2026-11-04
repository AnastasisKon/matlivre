//! Time zones for `datetime`: fixed UTC offsets and IANA zones with their daylight saving
//! history (from the `chrono-tz` database), plus `timezones`.

use crate::error::{rt, LangError};
use chrono::{Offset, TimeZone};
use chrono_tz::{OffsetComponents, OffsetName, Tz};

type R<T> = Result<T, LangError>;

const MS_MIN: f64 = 60_000.0;
const MS_HOUR: f64 = 3_600_000.0;

/// The zone names `timezones` lists, as MATLAB R2023b does.
const LISTED: &str = include_str!("tz_names.txt");

const AREAS: [&str; 11] = ["Africa", "America", "Antarctica", "Arctic", "Asia", "Atlantic", "Australia", "Etc", "Europe", "Indian", "Pacific"];

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Zone {
    /// A fixed offset from UTC, in milliseconds.
    Fixed(f64),
    Named(Tz),
}

/// Parses a `TimeZone` value: `''` (unzoned), `'UTC'`, `'+HH:mm'`, `'local'` or an IANA name.
/// Returns the zone and the name the `TimeZone` property shows.
pub fn parse(name: &str) -> R<Option<(Zone, String)>> {
    let n = name.trim();
    if n.is_empty() {
        return Ok(None);
    }
    let up = n.to_ascii_uppercase();
    if up == "UTC" || up == "GMT" || up == "Z" {
        return Ok(Some((Zone::Fixed(0.0), n.to_string())));
    }
    if up == "LOCAL" {
        let local = iana_time_zone::get_timezone().ok().and_then(|z| z.parse::<Tz>().ok());
        return Ok(Some(match local {
            Some(tz) => (Zone::Named(tz), tz.name().to_string()),
            None => (Zone::Fixed(crate::time::local_offset_ms()), "local".to_string()),
        }));
    }
    // IANA names are case-sensitive, as in MATLAB.
    if let Ok(tz) = n.parse::<Tz>() {
        return Ok(Some((Zone::Named(tz), tz.name().to_string())));
    }
    let off = up.strip_prefix("UTC").or_else(|| up.strip_prefix("GMT")).unwrap_or(&up);
    let bad = || LangError::new(format!("Unable to create the specified time zone: '{name}'. See the datetime.TimeZone property for details about specifying time zones."), 0);
    let (sign, rest) = match off.chars().next() {
        Some('+') => (1.0, &off[1..]),
        Some('-') => (-1.0, &off[1..]),
        _ => return Err(bad()),
    };
    let (h, m) = match rest.split_once(':') {
        Some((h, m)) => (h.parse::<f64>().ok(), m.parse::<f64>().ok()),
        None if rest.len() == 4 => (rest[..2].parse::<f64>().ok(), rest[2..].parse::<f64>().ok()),
        None => (rest.parse::<f64>().ok(), Some(0.0)),
    };
    match (h, m) {
        (Some(h), Some(m)) => Ok(Some((Zone::Fixed(sign * (h * MS_HOUR + m * MS_MIN)), n.to_string()))),
        _ => Err(bad()),
    }
}

fn instant(utc_ms: f64) -> Option<chrono::NaiveDateTime> {
    chrono::DateTime::from_timestamp_millis(utc_ms.floor() as i64).map(|d| d.naive_utc())
}

impl Zone {
    /// Offset of local time from UTC at an instant, in milliseconds.
    pub fn offset_at(&self, utc_ms: f64) -> f64 {
        match self {
            Zone::Fixed(o) => *o,
            Zone::Named(tz) => match instant(utc_ms) {
                Some(t) => tz.offset_from_utc_datetime(&t).fix().local_minus_utc() as f64 * 1000.0,
                None => 0.0,
            },
        }
    }

    /// The daylight saving part of the offset at an instant, in milliseconds. Zones the
    /// database models with a negative winter shift (Dublin, Casablanca) count, as in MATLAB,
    /// as standard time in winter and daylight saving time in summer.
    pub fn dst_at(&self, utc_ms: f64) -> f64 {
        let raw = |u: f64| match (self, instant(u)) {
            (Zone::Named(tz), Some(t)) => tz.offset_from_utc_datetime(&t).dst_offset().num_milliseconds() as f64,
            _ => 0.0,
        };
        let d = raw(utc_ms);
        if d != 0.0 {
            return d.max(0.0);
        }
        // Summer in a negative-shift zone: the shift is the size of its winter one.
        let half_year = 182.0 * 24.0 * MS_HOUR;
        let other = raw(utc_ms - half_year).min(raw(utc_ms + half_year));
        if other < 0.0 { -other } else { 0.0 }
    }

    /// The instant shown as wall-clock time `wall`. A time skipped by a daylight saving change
    /// moves forward by the gap and a repeated time takes its later occurrence, as in MATLAB.
    pub fn to_utc(&self, wall: f64) -> f64 {
        if wall.is_nan() {
            return f64::NAN;
        }
        let Zone::Named(_) = self else { return wall - self.offset_at(wall) };
        let mut offsets: Vec<f64> = [-36.0, 0.0, 36.0].iter().map(|h| self.offset_at(wall + h * MS_HOUR)).collect();
        for o in offsets.clone() {
            offsets.push(self.offset_at(wall - o));
        }
        let valid = offsets.iter().map(|o| wall - o).filter(|u| self.offset_at(*u) == wall - u).fold(f64::NEG_INFINITY, f64::max);
        if valid.is_finite() {
            valid
        } else {
            offsets.iter().map(|o| wall - o).fold(f64::NEG_INFINITY, f64::max)
        }
    }

    /// The short name the `z` format shows: North American abbreviations (EST, PDT, ...) and
    /// GMT, otherwise `UTC±H[:mm]`.
    pub fn short_name(&self, utc_ms: f64) -> String {
        if let (Zone::Named(tz), Some(t)) = (self, instant(utc_ms)) {
            let off = tz.offset_from_utc_datetime(&t);
            let abbr = off.abbreviation().unwrap_or("");
            let american = ["EST", "EDT", "CST", "CDT", "MST", "MDT", "PST", "PDT", "AKST", "AKDT", "HST", "HDT", "AST", "ADT"];
            if abbr == "GMT" || (american.contains(&abbr) && (tz.name().starts_with("America/") || tz.name().starts_with("US/") || tz.name() == "Pacific/Honolulu")) {
                return abbr.to_string();
            }
        }
        let off = self.offset_at(utc_ms);
        if off == 0.0 {
            return "UTC".into();
        }
        let (sign, a) = (if off < 0.0 { '-' } else { '+' }, off.abs());
        let (h, m) = ((a / MS_HOUR).floor() as i64, ((a % MS_HOUR) / MS_MIN).round() as i64);
        if m == 0 { format!("UTC{sign}{h}") } else { format!("UTC{sign}{h}:{m:02}") }
    }
}

/// Text for the offset tokens `Z`, `x` and `X` (`n` letters long) of a datetime format.
pub fn offset_token(c: char, n: usize, off_ms: f64) -> String {
    let (sign, a) = (if off_ms < 0.0 { '-' } else { '+' }, off_ms.abs());
    let (h, m) = ((a / MS_HOUR).floor() as i64, ((a % MS_HOUR) / MS_MIN).round() as i64);
    let basic = format!("{sign}{h:02}{m:02}");
    let extended = format!("{sign}{h:02}:{m:02}");
    let short = if m == 0 { format!("{sign}{h:02}") } else { basic.clone() };
    let zero = off_ms == 0.0;
    match (c, n) {
        ('Z', 1..=3) => basic,
        ('Z', 4) => if zero { "UTC".into() } else { format!("UTC{extended}") },
        ('Z', _) => if zero { "Z".into() } else { extended },
        ('x', 1) => short,
        ('x', 2) => basic,
        ('x', _) => extended,
        (_, 1) => if zero { "Z".into() } else { short },
        (_, 2) => if zero { "Z".into() } else { basic },
        _ => if zero { "Z".into() } else { extended },
    }
}

/// `timezones([area])`: names, areas and the standard and daylight saving offsets in hours.
pub fn timezones(area: Option<&str>) -> R<(Vec<String>, Vec<String>, Vec<f64>, Vec<f64>)> {
    if let Some(a) = area {
        if !AREAS.iter().any(|x| x.eq_ignore_ascii_case(a)) {
            return rt("AREA must be 'Africa', 'America', 'Antarctica', 'Arctic', 'Asia', 'Atlantic', 'Australia', 'Etc', 'Europe', 'Indian', or 'Pacific'");
        }
    }
    // Offsets as of the current year: the standard offset and the largest daylight saving shift.
    let now = crate::time::now_utc_ms();
    let year = 365.0 * 24.0 * MS_HOUR;
    let (mut names, mut areas, mut std_off, mut dst_off) = (Vec::new(), Vec::new(), Vec::new(), Vec::new());
    for name in LISTED.lines().map(str::trim).filter(|l| !l.is_empty()) {
        let zone_area = name.split('/').next().unwrap_or("");
        if area.is_some_and(|a| !a.eq_ignore_ascii_case(zone_area)) {
            continue;
        }
        let Ok(tz) = name.parse::<Tz>() else { continue };
        let samples: Vec<chrono::NaiveDateTime> = (0..53).filter_map(|w| instant(now - year + w as f64 * 7.0 * 24.0 * MS_HOUR)).collect();
        let mut base = samples.last().map(|t| tz.offset_from_utc_datetime(t).base_utc_offset().num_milliseconds() as f64).unwrap_or(0.0);
        let shifts: Vec<f64> = samples.iter().map(|t| tz.offset_from_utc_datetime(t).dst_offset().num_milliseconds() as f64).collect();
        let (lo, hi) = shifts.iter().fold((0.0f64, 0.0f64), |(lo, hi), &d| (lo.min(d), hi.max(d)));
        // A negative winter shift is standard time with summer daylight saving time.
        base += lo;
        let dst = hi - lo;
        names.push(name.to_string());
        areas.push(zone_area.to_string());
        std_off.push(base / MS_HOUR);
        dst_off.push(dst / MS_HOUR);
    }
    Ok((names, areas, std_off, dst_off))
}
