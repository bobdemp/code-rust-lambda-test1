//==============================================================================
//
// Rust SQS Lambda
//
// pip3 install cargo-lambda
//
// cargo lambda new rust-lambda-test1 && cd rust-lambda-test1
// cargo lambda watch
//
// http://localhost:9000/lambda-url/rust-lambda-test1/
// or
// cargo lambda invoke rust-lambda-test1 --data-file my-payload.json --env-vars FOO=BAR,BAZ=QUX
//
// cargo lambda build --output-format zip
// cargo lambda build --release --output-format zip
// set    RUST_LOG=info 
// set    RUST_LOG=debug 
// export RUST_LOG=info 
// export RUST_LOG=debug
//
//==============================================================================

use std::time::SystemTime;
use lambda_runtime::{service_fn, Error, LambdaEvent};
use aws_config::BehaviorVersion;
use serde::{Serialize};
use tracing_subscriber::filter::{EnvFilter, LevelFilter};

//////////////////////////////
// Config 
//////////////////////////////

#[derive(Debug, Serialize)]
struct Response {
    req_id: String,
    body: String,
}

impl std::fmt::Display for Response {
    /// Display the response struct as a JSON string
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let err_as_json = serde_json::json!(self).to_string();
        write!(f, "{err_as_json}")
    }
}

impl std::error::Error for Response {}

//////////////////////////////
// Core Functions
//////////////////////////////

#[doc = "Put S3 Object"]
async fn put_s3_object(s3_client: &aws_sdk_s3::Client, bucket_name: &str, event: aws_lambda_events::event::sqs::SqsMessage) -> Result<Response, Error> {
        
    
    let timestamp = SystemTime::now().duration_since(SystemTime::UNIX_EPOCH)
                                          .map(|n| n.as_secs())
                                          .expect("SystemTime before UNIX EPOCH, clock might have gone backwards");

    let filename = format!("data/kinesis-record-{timestamp}.json");
    let response = s3_client.put_object()
                            .bucket(bucket_name)
                            .body(event.as_deref().unwrap_or_default().as_bytes().to_owned().into())
                            .key(&filename)
                            .content_type("text/plain")
                            .send().await;

    match response {
        Ok(_) => {
            tracing::info!("File successfully stored in S3 - {}", filename);
            
            Ok(Response {
                req_id: event.message_id.as_deref().unwrap_or_default().to_string(),
                body: format!("File successfully stored in S3 - {}", filename),
            })
        }
        Err(e) => {
            tracing::error!("***** Error encountered: {:?} *****", e); 
            tracing::error!("File unsuccessfully stored in S3 - {}", filename);
            tracing::error!("File body - [{}]", event.body.as_deref().unwrap_or_default().to_string());

            Err(Box::new(Response {req_id: event.message_id.as_deref().unwrap_or_default().to_string(),
                                   body: format!("File unsuccessfully stored in S3 - {}", filename).to_owned()            
                                  }))

        }
    }
}

#[doc = "Process sqs message"]
async fn function_handler(event: LambdaEvent<aws_lambda_events::event::sqs::SqsEvent>) -> Result<(), Error> {
    
    tracing::info!("Started Process SQS Message(s)");
    tracing::info!("");
    tracing::info!("Checking environment variables");

    let bucket_name = std::env::var("S3_DEAD_LETTER_QUEUE_BUCKET_NAME").expect("Failed. A S3_DEAD_LETTER_QUEUE_BUCKET_NAME must be set in this Lambda environment variables.");

    // Initialize the client here to be able to reuse it across different invocations.
    
    let config = aws_config::load_defaults(BehaviorVersion::latest()).await;
    let s3_client = aws_sdk_s3::Client::new(&config);

    for record in event.payload.records { 
        
        tracing::info!("Message received - [{:?}]", record);
        
        let result = put_s3_object(&s3_client, &bucket_name, record).await;

        match result {
            Ok(_) => (),
            Err(e) => tracing::error!("***** Error encountered: {:?} *****", e)  
        };

    };

    tracing::info!("Completed Process SQS Message(s)");

    Ok(())
}

//////////////////////////////
// Main Functions
//////////////////////////////

#[tokio::main]
#[doc = "Main"]
async fn main() -> Result<(), Error> {

    tracing_subscriber::fmt()
        .with_env_filter(
            EnvFilter::builder()
                .with_default_directive(LevelFilter::INFO.into())
                .from_env_lossy(),
        )
        .with_target(false)
        .without_time()
        .init();

    lambda_runtime::run(service_fn(function_handler)).await

}