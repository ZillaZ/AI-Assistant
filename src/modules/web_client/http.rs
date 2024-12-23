pub fn get_header(headers: &[httparse::Header], target: &str) -> Option<String> {
    for header in headers.iter() {
        if header.name == target {
            return Some(String::from_utf8_lossy(header.value).to_string());
        }
    }
    None
}

pub fn new_headers<'a>(headers: &'a Vec<(&'a str, &'a str)>) -> Vec<httparse::Header<'a>> {
    headers
        .iter()
        .map(|(key, value)| httparse::Header {
            name: key,
            value: value.as_bytes(),
        })
        .collect::<Vec<httparse::Header>>()
}

pub fn response_to_string<T: AsRef<[u8]>>(response: httparse::Response, body: Option<T>) -> String {
    let code = format!(
        "HTTP/1.1 {} {}\n",
        response.code.unwrap(),
        response.reason.unwrap()
    );
    let mut headers = Vec::new();
    for header in response.headers.iter() {
        let header = format!(
            "{}: {}\n",
            header.name,
            String::from_utf8_lossy(header.value).to_string()
        );
        headers.push(header);
    }
    let code = code.as_bytes().to_vec();
    let headers = headers
        .iter()
        .map(|x| x.as_bytes().to_vec())
        .collect::<Vec<Vec<u8>>>();
    let headers = headers.concat();
    let mut response = vec![code, headers];
    if let Some(body) = body {
        response.push("\n".as_bytes().to_vec());
        response.push(body.as_ref().to_vec());
    }
    String::from_utf8_lossy(&response.concat()).to_string()
}
