use crate::tests::{fixtures, util};

#[tokio::test]
async fn test_asn_query() {
    let store = fixtures::get_new_store();

    util::import_file_in_store(&store, "data/arin_small.db", "arin").await;

    let mut query_result = store
        .query_asn(vec![String::from("arin")], String::from("AS14907"))
        .expect("test query did not sent back any routes")
        .clone();

    let mut wmd_prefixes = vec![
        String::from("198.35.26.0/24"),
        String::from("198.35.27.0/24"),
        String::from("208.80.152.0/23"),
        String::from("208.80.154.0/23"),
        String::from("2620:0:860::/48"),
        String::from("2620:0:861::/48"),
        String::from("2620:0:863::/48"),
    ];
    wmd_prefixes.sort();
    query_result.prefixes.sort();

    assert_eq!(query_result.prefixes, wmd_prefixes);
}
