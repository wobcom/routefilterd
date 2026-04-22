use crate::common_loader::{LoadFromURL, LoadFromURLError};
use crate::store_importer::*;
use mockall::mock;
use reqwest::Url;
use std::io::{BufRead, BufReader};
use stringreader::StringReader;

mock! {
    pub CommonLoader {
    }

    impl LoadFromURL<Box<dyn BufRead>> for CommonLoader {
        fn load_from_url(&self, url: &Url) -> Result<Box<dyn BufRead>, LoadFromURLError> {
        }
    }
}

#[test]
fn test_rpsl_preparser_iter() {
    let mut mockloader = MockCommonLoader::new();
    let url = Url::parse("http://localhost").unwrap();

    mockloader.expect_load_from_url().times(1).returning(|_| {
        Ok(Box::new(BufReader::new(StringReader::new(
            "
#
# The contents of this file are subject to
# RIPE Database Terms and Conditions
#
# https://docs.db.ripe.net/terms-conditions.html
#

as-set:         AS-RESTENA
descr:          Reseau Teleinformatique de l'Education Nationale
descr:          Educational and research network for Luxembourg
members:        AS2602
members:        AS42909
members:        AS51966
members:        AS-LXP
members:        AS-VDL
members:        AS112
tech-c:         DUMY-RIPE
admin-c:        DUMY-RIPE
notify:         noc@restena.lu
mnt-by:         AS2602-MNT
created:        1970-01-01T00:00:00Z
last-modified:  2024-08-08T12:58:40Z
source:         RIPE
remarks:        ****************************
remarks:        * THIS OBJECT IS MODIFIED
remarks:        * Please note that all data that is generally regarded as personal
remarks:        * data has been removed from this object.
remarks:        * To view the original object, please query the RIPE Database at:
remarks:        * http://www.ripe.net/whois
remarks:        ****************************

as-set:         AS-RENATER
descr:          RENATER
members:        AS261,    AS288,    AS513,    AS775,    AS776,    AS777,    AS781,    AS782
members:        AS789
members:        AS1300,   AS1303,   AS1304,   AS1307,   AS1712,   AS1715,   AS1717,   AS1724
members:        AS1725,   AS1726,   AS1935,   AS1936,   AS1937,   AS1938,   AS1939,   AS1940
members:        AS1941,   AS1942,   AS1943,   AS1944,   AS1945,   AS1948,   AS1951
members:        AS2060,   AS2065,   AS2067,   AS2072,   AS2085,   AS2088,   AS2089,   AS2094
members:        AS2103,   AS2187,   AS2188,   AS2193,   AS2194,   AS2198,   AS2199,   AS2200
members:        AS2202,   AS2222,   AS2223,   AS2231,   AS2236,   AS2239,   AS2258,   AS2259
members:        AS2263,   AS2264,   AS2269,   AS2418,   AS2422,   AS2426,   AS2439,   AS2445
members:        AS2450,   AS2457,   AS2462,   AS2470,   AS2471,   AS2472,   AS2475,   AS2484
members:        AS2485,   AS2486,   AS3557,   AS7500,   AS8674
members:        AS15655,  AS20144,  AS23634,  AS29110,  AS29199,  AS29216,  AS30126,  AS30839
members:        AS34000,  AS34542,  AS39444,  AS44850,  AS47300,  AS47608,  AS50897,  AS56774
members:        AS57284
members:        AS201659, AS202321, AS209136, AS215928
tech-c:         DUMY-RIPE
admin-c:        DUMY-RIPE
notify:         rensvp@renater.fr
mnt-by:         RENATER-MNT
remarks:        changed: rensvp@renater.fr 20000112
remarks:        changed: rensvp@renater.fr 20260319
created:        2001-11-12T10:11:50Z
last-modified:  2026-03-19T13:11:33Z
source:         RIPE
remarks:        ****************************
remarks:        * THIS OBJECT IS MODIFIED
remarks:        * Please note that all data that is generally regarded as personal
remarks:        * data has been removed from this object.
remarks:        * To view the original object, please query the RIPE Database at:
remarks:        * http://www.ripe.net/whois
remarks:        ****************************
",
        ))))
    });

    let mut parser = RpslParser::new_from_url(Box::new(mockloader), &url).unwrap();

    // TODO fix implementation, should not yield newlines, only objects
    assert_eq!(parser.next(), Some(String::from("\n")));
    assert_eq!(parser.next(), Some(String::from("\n")));

    assert_eq!(
        parser.next(),
        Some(String::from(
            "as-set:         AS-RESTENA
descr:          Reseau Teleinformatique de l'Education Nationale
descr:          Educational and research network for Luxembourg
members:        AS2602
members:        AS42909
members:        AS51966
members:        AS-LXP
members:        AS-VDL
members:        AS112
tech-c:         DUMY-RIPE
admin-c:        DUMY-RIPE
notify:         noc@restena.lu
mnt-by:         AS2602-MNT
created:        1970-01-01T00:00:00Z
last-modified:  2024-08-08T12:58:40Z
source:         RIPE
remarks:        ****************************
remarks:        * THIS OBJECT IS MODIFIED
remarks:        * Please note that all data that is generally regarded as personal
remarks:        * data has been removed from this object.
remarks:        * To view the original object, please query the RIPE Database at:
remarks:        * http://www.ripe.net/whois
remarks:        ****************************

"
        ))
    );

    assert_eq!(
        parser.next(),
        Some(String::from(
            "as-set:         AS-RENATER
descr:          RENATER
members:        AS261,    AS288,    AS513,    AS775,    AS776,    AS777,    AS781,    AS782
members:        AS789
members:        AS1300,   AS1303,   AS1304,   AS1307,   AS1712,   AS1715,   AS1717,   AS1724
members:        AS1725,   AS1726,   AS1935,   AS1936,   AS1937,   AS1938,   AS1939,   AS1940
members:        AS1941,   AS1942,   AS1943,   AS1944,   AS1945,   AS1948,   AS1951
members:        AS2060,   AS2065,   AS2067,   AS2072,   AS2085,   AS2088,   AS2089,   AS2094
members:        AS2103,   AS2187,   AS2188,   AS2193,   AS2194,   AS2198,   AS2199,   AS2200
members:        AS2202,   AS2222,   AS2223,   AS2231,   AS2236,   AS2239,   AS2258,   AS2259
members:        AS2263,   AS2264,   AS2269,   AS2418,   AS2422,   AS2426,   AS2439,   AS2445
members:        AS2450,   AS2457,   AS2462,   AS2470,   AS2471,   AS2472,   AS2475,   AS2484
members:        AS2485,   AS2486,   AS3557,   AS7500,   AS8674
members:        AS15655,  AS20144,  AS23634,  AS29110,  AS29199,  AS29216,  AS30126,  AS30839
members:        AS34000,  AS34542,  AS39444,  AS44850,  AS47300,  AS47608,  AS50897,  AS56774
members:        AS57284
members:        AS201659, AS202321, AS209136, AS215928
tech-c:         DUMY-RIPE
admin-c:        DUMY-RIPE
notify:         rensvp@renater.fr
mnt-by:         RENATER-MNT
remarks:        changed: rensvp@renater.fr 20000112
remarks:        changed: rensvp@renater.fr 20260319
created:        2001-11-12T10:11:50Z
last-modified:  2026-03-19T13:11:33Z
source:         RIPE
remarks:        ****************************
remarks:        * THIS OBJECT IS MODIFIED
remarks:        * Please note that all data that is generally regarded as personal
remarks:        * data has been removed from this object.
remarks:        * To view the original object, please query the RIPE Database at:
remarks:        * http://www.ripe.net/whois
remarks:        ****************************
"
        ))
    )
}
