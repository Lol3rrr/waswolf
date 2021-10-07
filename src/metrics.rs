use lazy_static::lazy_static;
use prometheus::Encoder;

lazy_static! {
    pub static ref REGISTRY: prometheus::Registry =
        prometheus::Registry::new_custom(Some("waswolf".to_string()), None).unwrap();
}

async fn handle(_req: hyper::Request<hyper::Body>) -> Result<hyper::Response<hyper::Body>, String> {
    let mut buffer = Vec::new();
    let encoder = prometheus::TextEncoder::new();

    let metrics = REGISTRY.gather();

    encoder.encode(&metrics, &mut buffer).unwrap();
    Ok(hyper::Response::new(hyper::Body::from(buffer)))
}

#[tracing::instrument]
pub async fn run_metrics_endpoint(port: u16) {
    let addr = std::net::SocketAddr::from(([0, 0, 0, 0], port));

    let make_service = hyper::service::make_service_fn(|_conn| async {
        Ok::<_, String>(hyper::service::service_fn(handle))
    });

    let server = hyper::Server::bind(&addr).serve(make_service);

    if let Err(e) = server.await {
        tracing::error!("Running Webserver: {:?}", e);
    }
}
