//! caldiff, nweekdate/lweekdate, leapseconds, tzoffset, time and the key/type helpers against
//! MATLAB R2023b.

use livre_lang::{Interpreter, OutputKind};

fn shown(src: &str) -> Vec<String> {
    let mut it = Interpreter::new();
    let _ = it.run_source(src);
    it.take_outputs()
        .into_iter()
        .filter_map(|o| match o.kind {
            OutputKind::Value { text, .. } => Some(text),
            OutputKind::Text(t) => Some(t),
            OutputKind::Error(e) => Some(format!("Error: {e}")),
            _ => None,
        })
        .collect()
}

#[test]
fn caldiff_matches_matlab() {
    assert_eq!(
        shown(concat!(
            "t = datetime(2020,1,[1 15 45]);\nd = caldiff(t)\nd4 = caldiff([t' t'])\n",
            "t = datetime(2020,[1 3 4],[31 1 15], [0 5 0], 0, 0);\nd = caldiff(t)\nd3 = caldiff(t, 'weeks')\nd4 = caldiff(t, 'quarters')\n",
            "d5 = caldiff(datetime(2019,1,1) + calyears(0:2))\nc0 = caldiff(datetime(2020,1,1))\n",
            "y = caldiff([datetime(2020,1,1) datetime(2021,3,2,1,2,3)], {'months','time'})\n",
            "tm = time(between(datetime(2020,1,1), datetime(2020,3,5,4,0,0)))\n",
            "try, caldiff(1:3), catch e, disp(e.message), end\n",
            "try, caldiff(t, 'foo'), catch e, disp(e.message), end\n",
        )),
        [
            "d = 1×2 calendarDuration array\n\n   14d   30d",
            "d4 = 2×2 calendarDuration array\n\n   14d   14d\n   30d   30d",
            "d = 1×2 calendarDuration array\n\n     1mo 1d 5h 0m 0s   1mo 13d 19h 0m 0s",
            "d3 = 1×2 calendarDuration array\n\n   4w   6w",
            "d4 = 1×2 calendarDuration array\n\n   0d   0d",
            "d5 = 1×2 calendarDuration array\n\n   1y   1y",
            "c0 = \n  0×1 empty calendarDuration array",
            "y = calendarDuration\n   14mo 25h 2m 3s",
            "tm = duration\n   04:00:00",
            "Undefined function 'caldiff' for input arguments of type 'double'.\n",
            "COMPONENTS must be 'Years', 'Quarters', 'Months', 'Weeks', 'Days', or 'Time', or a string array or cell array containing those components.\n",
        ]
    );
}

#[test]
fn weekdates_leapseconds_and_offsets() {
    assert_eq!(
        shown(concat!(
            "n = nweekdate(2, 2, 2024, 2)\nn2 = nweekdate(1, 6, 2024, 1:3)\nn3 = nweekdate(5, 2, 2024, 2)\nl = lweekdate(6, 2024, 1:3)\n",
            "ls = leapseconds;\nsz = size(ls)\nls(end,:)\n[~, v] = leapseconds; v\n",
            "z = tzoffset(datetime(2020,1,1,'TimeZone','+05:30'))\nz0 = tzoffset(datetime(2020,1,1))\n",
            "x = matlab.datetime.compatibility.convertDatenum(738000)\n",
        )),
        [
            "n = 739294",
            "n2 = 1×3\n\n      739256      739284      739312",
            "n3 = 0",
            "l = 1×3\n\n      739277      739305      739340",
            "sz = 1×2\n\n    27     2",
            "ans = 1×2 timetable\n\n       Date        Type    CumulativeAdjustment\n    ___________    ____    ____________________\n\n    31-Dec-2016     +             27 sec       ",
            "v = 65",
            "z = duration\n   05:30",
            "z0 = duration\n   NaN",
            "x = datetime\n   28-Jul-2020",
        ]
    );
}

#[test]
fn key_and_underlying_type_helpers() {
    assert_eq!(
        shown(concat!(
            "u = underlyingType(single(3))\nu3 = underlyingType(table(1))\nb = isUnderlyingType(int8(1), 'integer')\nb4 = isUnderlyingType(true, 'numeric')\n",
            "h = keyHash(1) == keyHash(1)\nk = keyHash(1) == keyHash(int8(1))\nc = class(keyHash('abc'))\n",
            "m = keyMatch(1, 1)\nm2 = keyMatch(\"a\", 'a')\nk3 = keyMatch({1}, {1})\n",
        )),
        [
            "u = 'single'",
            "u3 = 'table'",
            "b = logical\n   1",
            "b4 = logical\n   0",
            "h = logical\n   1",
            "k = logical\n   0",
            "c = 'uint64'",
            "m = logical\n   1",
            "m2 = logical\n   0",
            "k3 = logical\n   1",
        ]
    );
}
