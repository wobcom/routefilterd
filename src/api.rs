use crate::store::PoofData;
use http::StatusCode;
use std::time::Instant;

pub async fn get_route_from_as_set(
    name: String,
    data: PoofData,
) -> Result<impl warp::Reply, warp::Rejection> {
    let old_time = Instant::now();
    if let Some(value) = data.query_as_set_prefixes_recursive(name.to_string(), 25) {
        Ok(warp::reply::with_status(
            format!(
                "# Recursed route resolution for '{}' in {}μs, {} items\n{:#?}",
                name,
                old_time.elapsed().as_micros(),
                value.len(),
                value
            ),
            StatusCode::OK,
        ))
    } else {
        Ok(warp::reply::with_status(
            format!("Value for '{}' not found in cache.", name),
            StatusCode::NOT_FOUND,
        ))
    }
}

pub async fn get_asn_from_as_set(
    name: String,
    data: PoofData,
) -> Result<impl warp::Reply, warp::Rejection> {
    let old_time = Instant::now();
    if let Some(value) = data.query_as_set_recursive(name.to_string(), 25) {
        Ok(warp::reply::with_status(
            format!(
                "# Recursed AS-Set resolution for '{}' in {}μs, {} items\n{:#?}",
                name,
                old_time.elapsed().as_micros(),
                value.len(),
                value
            ),
            StatusCode::OK,
        ))
    } else {
        Ok(warp::reply::with_status(
            format!("Value for '{}' not found in cache.", name),
            StatusCode::NOT_FOUND,
        ))
    }
}
