use jsonrpsee::{http_client::HttpClientBuilder, types::traits::Client};

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    // Assuming that the snarkOS is running locally, by default 3032 is the rpc server port
    let url = format!("http://{}", "0.0.0.0:3032");
    let client = HttpClientBuilder::default().build(url)?;
    let response: Result<String, _> = client.request("latestblock", None).await;
    println!("response: {:?}", response);
    Ok(())
}

#[cfg(test)]
mod tests {
    use json_rpc_types::Request;
    use jsonrpc_core::Params;
    use jsonrpsee_types::v2::{Id, Request as SeeRequest, RequestSer};
    use serde_json;

    #[test]
    fn it_serde() {
        let id = Id::Number(0);
        let from_eclipse = RequestSer::new(id, "latestblock", None);
        let from_eclipse_str = serde_json::to_string(&from_eclipse).unwrap();

        // Print out from the Aleo node receiving the request before parsing
        // b"{\"jsonrpc\":\"2.0\",\"id\":0,\"method\":\"getblock\"}";
        let aleo_req: Request<Params> =
            serde_json::from_slice(&from_eclipse_str.as_bytes()).unwrap();

        let aleo_req_str = serde_json::to_string(&aleo_req).unwrap();
        let parsed_aleo_req: SeeRequest = serde_json::from_slice(&aleo_req_str.as_bytes()).unwrap();

        assert_eq!(parsed_aleo_req.method, from_eclipse.method);
        assert_eq!(parsed_aleo_req.id, from_eclipse.id);
        assert_eq!(
            parsed_aleo_req.params.is_none(),
            from_eclipse.params.is_none()
        );
        assert_eq!(serde_json::to_string(&aleo_req).unwrap(), from_eclipse_str);
    }
}
