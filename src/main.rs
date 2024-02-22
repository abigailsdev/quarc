use std::env;
use reqwest::{blocking, header};
use warc::{WarcWriter, WarcHeader, RecordType, Record, BufferedBody};
use soup::prelude::*;
use url::Url;

#[derive(Clone, Copy)]
struct Nothing;

fn grab_resource(url: String) -> Result<(Vec<u8>, Option<String>), u8> {
    // Grab with reqwest and turn the body into a vector of bytes
    // If there is a resouce type header, include the MIME as the option string

    println!("Fetching {}.", url);

    let raw_response = blocking::get(url);
    if !raw_response.is_ok() { return Err(1); }
    let response = raw_response.unwrap();
    if !response.status().is_success() { return Err(2); }

    let headers = response.headers();

    let mime;
    if headers.contains_key(header::CONTENT_TYPE)
    {
        let headervalue = headers[header::CONTENT_TYPE].to_str();
        if headervalue.is_ok()
        {
            mime = Some(headervalue.unwrap().to_string());
        } else {
            mime = None;
        }
    } else {
        mime = None;
    }

    let body = response.bytes();
    if !body.is_ok() { return Err(3); }
 
    return Ok((body.unwrap().to_vec(), mime));
}

fn main() {
    let args: Vec<String> = env::args().collect();
    if args.len() <= 1 { return; }
    let proc = Url::parse(&args[1]).expect("Invalid URL");
    let body;
    let mime;
    (body, mime) = grab_resource(args[1].to_string()).unwrap();

    let mut warc_head = Record::<BufferedBody>::new();
    let _ = warc_head.set_warc_type(RecordType::Resource);
    warc_head.set_header(WarcHeader::TargetURI, args[1].to_string());

    if mime.is_some() {
        warc_head.set_header(WarcHeader::ContentType, mime.unwrap());
    }
    
    let record = warc_head.add_body(body.clone());

    let body_text = String::from_utf8_lossy(&body);
    let mut dependent_records = vec![];
    let mut dependent_urls = vec![];

    let soup = Soup::new(&body_text.to_string());
    let links = soup.tag("link").find_all();
    for link in links {
        if link.get("rel").unwrap_or("".to_string()) == "me".to_string() { continue; }
        if link.get("href").is_some() {
            let mut nurl = link.get("href").unwrap().to_string();
            if nurl.starts_with("/") { nurl = format!("{}://{}{}", proc.scheme(), proc.host_str().unwrap(), nurl); }
            dependent_urls.push(nurl);
        }
    }
    let imgs = soup.tag("img").find_all();
    for img in imgs {
        if img.get("src").is_some() {
            let mut nurl = img.get("src").unwrap().to_string();
            if nurl.starts_with("/") { nurl = format!("{}://{}{}", proc.scheme(), proc.host_str().unwrap(), nurl); }
            dependent_urls.push(nurl);
        }
    }

    for url in dependent_urls {
        let dep_body;
        let dep_mime;

        (dep_body, dep_mime) = grab_resource(url.to_string()).unwrap_or((vec![], Some("".to_string())));
        
        if dep_body.len() == 0 {
            continue;
        }

        let mut dep_warc_head = Record::<BufferedBody>::new();
        let _ = dep_warc_head.set_warc_type(RecordType::Resource);
        dep_warc_head.set_header(WarcHeader::TargetURI, url.to_string());

        if dep_mime.is_some() {
            dep_warc_head.set_header(WarcHeader::ContentType, dep_mime.unwrap());
        }
    
        let dep_record = dep_warc_head.add_body(dep_body);
        dependent_records.push(dep_record);
    }

    let mut warc = WarcWriter::from_path("output.warc");

    if warc.is_ok() {
        let mut nwarc = warc.unwrap();
        nwarc.write(&record);
        for rec in dependent_records {
            nwarc.write(&rec);
        }
    } else {
        println!("Failed to open WARC file.");
        return;
    }
}
