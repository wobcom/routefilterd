use crate::tests::{fixtures, util};

const WMF_AS_SET: &str = "AS-WIKIMEDIA-ARIN";
const WMF_PREFIXES: &[&str] = &[
    "198.35.26.0/24",
    "198.35.27.0/24",
    "208.80.152.0/23",
    "208.80.154.0/23",
    "2620:0:860::/48",
    "2620:0:861::/48",
    "2620:0:863::/48",
];
const TELEMEDIA_AS_SET: &str = "AS-TELEMEDIA";

const TELEMEDIA_ASNS: &[&str] = &["AS16625", "AS132091", "AS17810", "AS132075", "AS45284"];
const TELEMEDIA_AS_SETS: &[&str] = &["AS-DATACON-IN", "AS55353:AS-RPNET-RACKBANK-GIGABITPORT"];
const WMF_HQ_AS: &str = "AS11820";
const WMF_WP_AS: &str = "AS14907";
const RESERVED_PRIV_AS: &str = "AS65534";
const UNKNOWN_AS_SET: &str = "AS-WOBCOM3000";

#[tokio::test]
async fn test_asn_query() {
    let store = fixtures::get_new_store();

    util::import_file_in_store(&store, "data/arin_small.db", "arin", 200).await;

    let query_result = store
        .query_asn(vec![String::from("arin")], String::from(WMF_WP_AS))
        .expect("test query did not sent back any routes")
        .clone();

    assert_eq!(
        util::sorted_vec(query_result.prefixes),
        util::sorted_vec(Vec::from(WMF_PREFIXES))
    );
}

#[tokio::test]
#[should_panic]
async fn test_asn_query_no_result() {
    let store = fixtures::get_new_store();

    util::import_file_in_store(&store, "data/arin_small.db", "arin", 200).await;

    store
        .query_asn(vec![String::from("arin")], String::from(RESERVED_PRIV_AS))
        .expect("test query did not sent back any routes");
}

#[tokio::test]
async fn test_as_set_shallow_query() {
    let wmd_as_set = String::from(WMF_AS_SET);

    let store = fixtures::get_new_store();

    util::import_file_in_store(&store, "data/arin_small.db", "arin", 200).await;

    let query_result = store
        .query_as_set(vec![], wmd_as_set.clone(), &[])
        .expect("query did not sent back any as-set");

    assert_eq!(
        util::sorted_vec(query_result.asns),
        util::sorted_vec(vec![WMF_WP_AS, WMF_HQ_AS])
    );
    assert_eq!(query_result.as_sets.len(), 0); // not recursive
}

#[tokio::test]
#[should_panic]
async fn test_as_set_shallow_query_no_result() {
    let unknown_as_set = String::from(UNKNOWN_AS_SET);

    let store = fixtures::get_new_store();

    util::import_file_in_store(&store, "data/arin_small.db", "arin", 200).await;

    store
        .query_as_set(vec![], unknown_as_set.clone(), &[])
        .expect("query did not sent back any as-set");
}

#[tokio::test]
async fn test_as_set_overlapping_datasources() {
    let telemedia_as_set = String::from(TELEMEDIA_AS_SET);

    let store = fixtures::get_new_store();

    // redundant definition, radb also has another telemedia
    util::import_file_in_store(&store, "data/radb_small.db", "radb", 100).await;
    util::import_file_in_store(&store, "data/apnic_small.db", "apnic", 200).await;

    let query_result = store
        .query_as_set(vec![], telemedia_as_set.clone(), &[])
        .expect("query did not sent back any result");

    assert_eq!(
        util::sorted_vec(query_result.asns),
        util::sorted_vec(Vec::from(TELEMEDIA_ASNS))
    );

    assert_eq!(
        util::sorted_vec(query_result.as_sets),
        util::sorted_vec(Vec::from(TELEMEDIA_AS_SETS))
    );
}

#[tokio::test]
async fn test_as_set_overlapping_datasources_w_exclude() {
    let telemedia_as_set = String::from(TELEMEDIA_AS_SET);

    let store = fixtures::get_new_store();

    // redundant definition, radb also has another telemedia
    util::import_file_in_store(&store, "data/radb_small.db", "radb", 200).await;
    util::import_file_in_store(&store, "data/apnic_small.db", "apnic", 100).await;

    let query_result = store
        .query_as_set(vec![], telemedia_as_set.clone(), &[String::from("radb")])
        .expect("query did not sent back any result");

    assert_eq!(
        util::sorted_vec(query_result.asns),
        util::sorted_vec(Vec::from(TELEMEDIA_ASNS))
    );

    assert_eq!(
        util::sorted_vec(query_result.as_sets),
        util::sorted_vec(Vec::from(TELEMEDIA_AS_SETS))
    );
}

#[tokio::test]
async fn test_as_set_overlapping_datasources_w_include() {
    let telemedia_as_set = String::from(TELEMEDIA_AS_SET);

    let store = fixtures::get_new_store();

    // redundant definition, radb also has another telemedia
    util::import_file_in_store(&store, "data/radb_small.db", "radb", 200).await;
    util::import_file_in_store(&store, "data/apnic_small.db", "apnic", 100).await;

    let query_result = store
        .query_as_set(vec![String::from("apnic")], telemedia_as_set.clone(), &[])
        .expect("query did not sent back any result");

    assert_eq!(
        util::sorted_vec(query_result.asns),
        util::sorted_vec(Vec::from(TELEMEDIA_ASNS))
    );

    assert_eq!(
        util::sorted_vec(query_result.as_sets),
        util::sorted_vec(Vec::from(TELEMEDIA_AS_SETS))
    );
}
