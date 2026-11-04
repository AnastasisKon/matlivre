//! Named time zones, daylight saving time, zone format fields, tzoffset, isdst and timezones
//! against MATLAB R2023b.

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
fn zones_convert_and_follow_daylight_saving() {
    assert_eq!(
        shown(concat!(
            "d = datetime(2020,7,1,12,0,0,'TimeZone','America/New_York')\ntzn = d.TimeZone\nd.TimeZone = 'Europe/Paris'\nh = d.Hour\n",
            "d2 = datetime(2020,3,8,1,30,0,'TimeZone','America/New_York') + hours(1)\n",
            "d3 = datetime(2020,3,7,2,30,0,'TimeZone','America/New_York') + caldays(1)\n",
            "d4 = datetime(2020,3,8,2,30,0,'TimeZone','America/New_York')\n",
            "d5b = datetime(2020,11,1,1,30,0,'TimeZone','America/New_York') + hours(1)\n",
            "dd = datetime(2020,3,9,'TimeZone','America/New_York') - datetime(2020,3,8,'TimeZone','America/New_York')\n",
            "b = datetime(2020,1,1,'TimeZone','Europe/Paris') == datetime(2019,12,31,23,0,0,'TimeZone','UTC')\n",
            "r2 = datetime(2020,3,8,0,0,0,'TimeZone','America/New_York') + hours(0:3)\n",
            "u = datetime(2020,7,1,'TimeZone','America/New_York'); u.TimeZone = ''; u\n",
            "c = [datetime(2020,1,1,'TimeZone','Europe/Paris') datetime(2020,1,1,'TimeZone','UTC')]\n",
            "w = datetime(2020,7,1,12,0,0,'TimeZone','Europe/Paris') - datetime(2020,7,1,12,0,0,'TimeZone','America/New_York')\n",
            "TT = timetable(datetime(2020,3,7,'TimeZone','America/New_York') + hours(0:12:48)', (1:5)');\nR = retime(TT, 'daily', 'sum')\n",
        )),
        [
            "d = datetime\n   01-Jul-2020 12:00:00",
            "tzn = 'America/New_York'",
            "d = datetime\n   01-Jul-2020 18:00:00",
            "h = 18",
            "d2 = datetime\n   08-Mar-2020 03:30:00",
            "d3 = datetime\n   08-Mar-2020 03:30:00",
            "d4 = datetime\n   08-Mar-2020 03:30:00",
            "d5b = datetime\n   01-Nov-2020 02:30:00",
            "dd = duration\n   23:00:00",
            "b = logical\n   1",
            "r2 = 1×4 datetime array\n\n   08-Mar-2020 00:00:00   08-Mar-2020 01:00:00   08-Mar-2020 03:00:00   08-Mar-2020 04:00:00",
            "u = datetime\n   01-Jul-2020",
            "c = 1×2 datetime array\n\n   01-Jan-2020 00:00:00   01-Jan-2020 01:00:00",
            "w = duration\n   -06:00:00",
            "R = 3×1 timetable\n\n       Time        Var1\n    ___________    ____\n\n    07-Mar-2020     3  \n    08-Mar-2020     7  \n    09-Mar-2020     5  ",
        ]
    );
}

#[test]
fn offsets_dst_flags_and_errors() {
    assert_eq!(
        shown(concat!(
            "z = tzoffset(datetime(2020,7,1,'TimeZone','America/New_York'))\n",
            "[z, ds] = tzoffset(datetime(2020,[1 7],1,'TimeZone','Europe/Paris'))\n",
            "z5 = tzoffset(datetime(2020,11,1,1,30,0,'TimeZone','America/New_York'))\n",
            "z1 = tzoffset(datetime(1950,7,1,'TimeZone','America/New_York'))\nz3 = tzoffset(datetime(2018,1,1,'TimeZone','America/Sao_Paulo'))\n",
            "i = isdst(datetime(2020,[1 7],1,'TimeZone','Europe/Paris'))\ni0 = isdst(datetime(2020,7,1))\n",
            "try, datetime(2020,1,1,'TimeZone','Mars/Base'), catch e, disp(e.message), end\n",
            "try, datetime(2020,1,1,'TimeZone','america/new_york'), catch e, disp(e.message), end\n",
            "try, datetime(2020,1,1,'TimeZone','UTC') < datetime(2020,1,1), catch e, disp(e.message), end\n",
        )),
        [
            "z = duration\n   -04:00",
            "z = 1×2 duration array\n\n   01:00   02:00",
            "ds = 1×2 duration array\n\n   00:00   01:00",
            "z5 = duration\n   -05:00",
            "z1 = duration\n   -04:00",
            "z3 = duration\n   -02:00",
            "i = 1×2 logical array\n\n   0   1",
            "i0 = logical\n   0",
            "Unable to create the specified time zone: 'Mars/Base'. See the datetime.TimeZone property for details about specifying time zones.\n",
            "Unable to create the specified time zone: 'america/new_york'. See the datetime.TimeZone property for details about specifying time zones.\n",
            "Cannot combine or compare a datetime array with a time zone with one without a time zone.\n",
        ]
    );
}

#[test]
fn zone_format_fields_match_matlab() {
    let fmt = "z|Z|ZZZZ|ZZZZZ|x|xx|xxx|X|XXX";
    let cases = [
        ("Europe/London", 1, "GMT|+0000|UTC|Z|+00|+0000|+00:00|Z|Z"),
        ("Europe/London", 7, "UTC+1|+0100|UTC+01:00|+01:00|+01|+0100|+01:00|+01|+01:00"),
        ("America/Toronto", 7, "EDT|-0400|UTC-04:00|-04:00|-04|-0400|-04:00|-04|-04:00"),
        ("America/Anchorage", 1, "AKST|-0900|UTC-09:00|-09:00|-09|-0900|-09:00|-09|-09:00"),
        ("Australia/Sydney", 1, "UTC+11|+1100|UTC+11:00|+11:00|+11|+1100|+11:00|+11|+11:00"),
        ("America/St_Johns", 7, "UTC-2:30|-0230|UTC-02:30|-02:30|-0230|-0230|-02:30|-0230|-02:30"),
        ("Asia/Kathmandu", 1, "UTC+5:45|+0545|UTC+05:45|+05:45|+0545|+0545|+05:45|+0545|+05:45"),
        ("UTC", 1, "UTC|+0000|UTC|Z|+00|+0000|+00:00|Z|Z"),
        ("-03:00", 7, "UTC-3|-0300|UTC-03:00|-03:00|-03|-0300|-03:00|-03|-03:00"),
    ];
    for (zone, month, want) in cases {
        let out = shown(&format!("d = datetime(2020,{month},1,12,0,0,'TimeZone','{zone}','Format','{fmt}');\ndisp(char(d))\n"));
        assert_eq!(out, [format!("{want}\n")], "{zone} in month {month}");
    }
}

#[test]
fn timezones_lists_matlab_zones() {
    assert_eq!(
        shown("T = timezones; sz = size(T)\nv = T.Properties.VariableNames\nT(1:3,:)\nT2 = timezones('Europe'); sz2 = size(T2)\ntry, timezones('Foo'), catch e, disp(e.message), end\n"),
        [
            "sz = 1×2\n\n   457     4",
            "v = 1×4 cell\n\n    {'Name'}    {'Area'}    {'UTCOffset'}    {'DSTOffset'}",
            "ans = 3×4 table\n\n             Name              Area     UTCOffset    DSTOffset\n    ______________________    ______    _________    _________\n\n    {'Africa/Abidjan'    }    Africa        0            0    \n    {'Africa/Accra'      }    Africa        0            0    \n    {'Africa/Addis_Ababa'}    Africa        3            0    ",
            "sz2 = 1×2\n\n    60     4",
            "AREA must be 'Africa', 'America', 'Antarctica', 'Arctic', 'Asia', 'Atlantic', 'Australia', 'Etc', 'Europe', 'Indian', or 'Pacific'\n",
        ]
    );
}
