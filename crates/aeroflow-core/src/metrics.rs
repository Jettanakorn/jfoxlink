use prometheus::{
    HistogramOpts, Histogram, Counter, Gauge, register_counter, register_gauge, register_histogram,
};

macro_rules! metric {
    ($name:ident: Counter => $desc:expr) => {
        lazy_static::lazy_static! {
            pub static ref $name: Counter = register_counter!(
                concat!("aeroflow_", stringify!($name)),
                concat!("aeroflow_", stringify!($name), " ", $desc)
            ).expect(concat!("register ", stringify!($name)));
        }
    };
    ($name:ident: Gauge => $desc:expr) => {
        lazy_static::lazy_static! {
            pub static ref $name: Gauge = register_gauge!(
                concat!("aeroflow_", stringify!($name)),
                concat!("aeroflow_", stringify!($name), " ", $desc)
            ).expect(concat!("register ", stringify!($name)));
        }
    };
    ($name:ident: Histogram[$($buckets:expr),+] => $desc:expr) => {
        lazy_static::lazy_static! {
            pub static ref $name: Histogram = register_histogram!(
                HistogramOpts::new(
                    concat!("aeroflow_", stringify!($name)),
                    concat!("aeroflow_", stringify!($name), " ", $desc),
                )
                .buckets(vec![$($buckets),+])
            ).expect(concat!("register ", stringify!($name)));
        }
    };
}

metric!(cases_created: Counter => "Total cases created");
metric!(cases_completed: Counter => "Total cases completed");
metric!(cases_failed: Counter => "Total cases failed");
metric!(stl_files_imported: Counter => "Total STL files imported");
metric!(geometries_fingerprinted: Counter => "Total geometries fingerprinted");
metric!(skill_trials_run: Counter => "Total skill optimization trials");
metric!(mesh_quality_failures: Counter => "Total mesh quality failures");
metric!(http_requests: Counter => "Total HTTP requests");

metric!(active_cases: Gauge => "Currently running cases");
metric!(queue_depth: Gauge => "Cases waiting in queue");

metric!(pipeline_duration: Histogram[10.0, 30.0, 60.0, 120.0, 300.0, 600.0, 1800.0, 3600.0] => "Pipeline duration seconds");
metric!(solver_iterations: Histogram[100.0, 500.0, 1000.0, 2000.0, 5000.0, 10000.0] => "Solver iterations per case");
metric!(http_request_duration: Histogram[0.005, 0.01, 0.025, 0.05, 0.1, 0.25, 0.5, 1.0, 2.5] => "HTTP request duration seconds");
metric!(db_query_duration: Histogram[0.001, 0.005, 0.01, 0.025, 0.05, 0.1, 0.25, 0.5] => "DB query duration seconds");

pub fn gather_metrics() -> String {
    use prometheus::Encoder;
    let encoder = prometheus::TextEncoder::new();
    let mut buffer = Vec::new();
    encoder.encode(&prometheus::default_registry().gather(), &mut buffer)
        .expect("encode prometheus metrics");
    String::from_utf8(buffer).expect("prometheus output is valid utf-8")
}
