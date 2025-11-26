#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let client = reqwest::Client::builder()
        .build()?;

    let mut headers = reqwest::header::HeaderMap::new();
    headers.insert("Content-Type", "application/json".parse()?);

    let data = r#"{
    "data": "ChC4upvlOpNL3qQSTOGhirCyEiBSYW5kb20gTXVzaWMgR2VucmVzIFJhbmsgTGlzdCAjMRoUKhIKELr9vckldkDnvsoGvb+ZoZgaMwoxChC6/b3JJXZA577KBr2/maGYEh0KEKEmylMMjkjVuIiCxzTDiTUSCXJhbmtfdHlwZRpcEloKEORZtc3Xt0QguQmbKOuRtRoSEI8VG6TeIE48nLSZ3flvSPEaELr9vckldkDnvsoGvb+ZoZgyEICKBM6yHE2IitEuJAYT5cpKEGM3TLUnCEC/qzrYcnXRRPIaFioUChC1H2/IVWpA0LqHYWqSpibvEAUaLwotChC1H2/IVWpA0LqHYWqSpibvEhkKEKEmylMMjkjVuIiCxzTDiTUSBXJhbmtzGlwSWgoQraX2YpqzSEKtp/aT+CCLhRIQjxUbpN4gTjyctJnd+W9I8RoQtR9vyFVqQNC6h2FqkqYm7zIQgIoEzrIcTYiK0S4kBhPlykoQ9MN977LzQSe0kOXO4/z+9RoWKhQKEGZdcxrub0adgdIR2nJ8os8QARovCi0KEGZdcxrub0adgdIR2nJ8os8SGQoQoSbKUwyOSNW4iILHNMOJNRIFc2NvcmUaXBJaChAwaaZZUtdDu7ny9VBvqgOCEhCPFRuk3iBOPJy0md35b0jxGhBmXXMa7m9GnYHSEdpyfKLPMhCAigTOshxNiIrRLiQGE+XKShDje+05WPZKtYJQErK0EhQ7IhUAAAAAAAAAAAAAAAAAAAAAAAAAAAA="
}"#;

    let json: serde_json::Value = serde_json::from_str(&data)?;

    let request = client.request(reqwest::Method::POST, "http://127.0.0.1:8080/cache")
        .headers(headers)
        .json(&json);

    let response = request.send().await?;
    let body = response.text().await?;

    println!("{}", body);

    Ok(())
}