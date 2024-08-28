use anyhow::{anyhow, Result};
use core::panic;
use reqwest::blocking::multipart::{Form, Part};
use serde::Deserialize;
use std::error::Error;
use std::fs::File;
use std::io::{Read, Write};
use zip::write::SimpleFileOptions;
use zip::ZipWriter;

struct Client {
    client: reqwest::blocking::Client,
    base_url: String,
}

impl Client {
    fn get(&self, api_path: &str) -> reqwest::blocking::RequestBuilder {
        self.client.get(format!("{}/{}", &self.base_url, api_path))
    }

    fn post(&self, api_path: &str) -> reqwest::blocking::RequestBuilder {
        self.client.post(format!("{}/{}", &self.base_url, api_path))
    }
}

fn file_station_upload_error_str(code: i64) -> String {
    match code {
    1800 => "There is no Content-Length information in the HTTP header or the receved size doesn't match the value of Content-Length information in the HTTP header.",
    1801 => "Wait too long, no date can be receved from client. (Default maximum wait time is 3600 seconds).",
    1802 => "No filename information in the last part of file content.",
    1803 => "Upload connection is cancelled.",
    1804 => "Failed to upload oversized file to FAT file system.",
    1805 => "Can't overwrite or skip the existed file, if no `overwrite` parameter is given.",
    _ => return file_station_common_error_str(code)
    }.into()
}

fn file_station_common_error_str(code: i64) -> String {
    match code {
        400 => "Invalid parameter of file operation",
        401 => "Unknown error of file operation",
        402 => "System is too busy",
        403 => "Invalid user does this file operation",
        404 => "Invalid group does this file operation",
        405 => "Invalid user and group does this file operation",
        406 => "Can't get user/group information from the account server",
        407 => "Operation not permitted",
        408 => "No such file or directory",
        409 => "Non-supported file system",
        410 => "Failed to connect internet-based file system (ex: CIFS)",
        411 => "Read-only file system",
        412 => "Filename too long in the non-encrypted file system",
        413 => "Filename too long in the encrypted file system",
        414 => "File already exists",
        415 => "Disk quota exceeded",
        416 => "No space left on device",
        417 => "Input/output error",
        418 => "Illegal name or path",
        419 => "Illegal file name",
        420 => "Illegal file name on FAT file system",
        421 => "Device or resource busy",
        599 => "No such task of the file operation",
        _ => return format_common_error(code),
    }
    .into()
}

fn format_common_error(code: i64) -> String {
    match code {
        100 => "Unknown error",
        101 => "No parameter of API, method or version",
        102 => "The requested API does not exist",
        103 => "The requested method does not exist",
        104 => "The requested version does not support the functionality",
        105 => "The logged in session does not have permission",
        106 => "Session timeout",
        107 => "Session interrupted by duplicate login",
        119 => "SID not found",
        _ => "Error code unknown",
    }
    .into()
}

fn auth_error_str(code: i64) -> String {
    match code {
        400 => "No such account or incorrect password",
        401 => "Account disabled",
        402 => "Permission denied",
        403 => "2-step verification code required",
        404 => "Failed to authenticate 2-step verification code",
        _ => return format_common_error(code),
    }
    .into()
}

fn format_error_response(api_name: &str, resp: SynoResponse) -> anyhow::Error {
    let code = resp
        .error
        .expect("Cannot format error if there is no error!")
        .as_object()
        .unwrap()
        .get("code")
        .expect("All error objects must have an error code")
        .as_i64()
        .unwrap();
    let error_str = match api_name {
        "SYNO.API.Auth" => auth_error_str(code),
        "SYNO.FileStation.List" => file_station_common_error_str(code),
        "SYNO.FileStation.Upload" => file_station_upload_error_str(code),
        _ => panic!("Unknown API name"),
    };
    anyhow!("{} - {}", code, error_str)
}

#[derive(Debug)]
struct ApiInfo {
    min_version: u8,
    max_version: u8,
    path: String,
    name: String,
}

fn get_api_versions(client: &Client) -> Result<Vec<ApiInfo>> {
    let api_name = "SYNO.API.Info";
    let version = 1;
    let method = "query";
    let api_path = "query.cgi";

    let resp = client.get(api_path)
        .query(&[("api", api_name), ("version", &version.to_string()), ("method", method), ("query", "SYNO.API.Info,SYNO.API.Auth,SYNO.FileStation.Info,SYNO.FileStation.Upload,SYNO.FileStation.List")])
        .send()
        .unwrap().json::<SynoResponse>().unwrap();
    if resp.success {
        let data = resp
            .data
            .unwrap()
            .as_object()
            .unwrap()
            .iter()
            .map(|(k, v)| {
                let path = v.get("path").unwrap().as_str().unwrap().to_string();
                let name = k.to_string();
                let min_version = v.get("minVersion").unwrap().as_u64().unwrap() as u8;
                let max_version = v.get("maxVersion").unwrap().as_u64().unwrap() as u8;
                ApiInfo {
                    min_version,
                    max_version,
                    path,
                    name,
                }
            })
            .collect::<Vec<ApiInfo>>();
        Ok(data)
    } else {
        Err(format_error_response(api_name, resp))
    }
}

fn login(client: &Client, api: &[ApiInfo], passwd: &str, account: &str) -> Result<()> {
    let api_name = "SYNO.API.Auth";
    let version = 3;
    let method = "login";
    let api = api.iter().find(|x| x.name == api_name).unwrap();
    assert!(api.name == api_name);
    assert!(version <= api.max_version);
    assert!(api.min_version <= version);

    let resp = client
        .get(&api.path)
        .query(&[
            ("api", api_name),
            ("version", &version.to_string()),
            ("method", method),
            ("account", account),
            ("passwd", passwd),
            ("format", "cookie"),
        ])
        .send()
        .unwrap()
        .json::<SynoResponse>()
        .unwrap();
    if resp.success {
        Ok(())
    } else {
        Err(format_error_response(api_name, resp))
    }
}

fn logout(client: &Client, api: &[ApiInfo]) -> Result<()> {
    let api_name = "SYNO.API.Auth";
    let version = 3;
    let method = "logout";
    let api = api.iter().find(|x| x.name == api_name).unwrap();
    let resp = client
        .get(&api.path)
        .query(&[
            ("api", api_name),
            ("version", &version.to_string()),
            ("method", method),
            ("format", "cookie"),
        ])
        .send()
        .unwrap()
        .json::<SynoResponse>()
        .unwrap();
    if resp.success {
        Ok(())
    } else {
        Err(format_error_response(api_name, resp))
    }
}

#[derive(Debug)]
struct SharedFolder {
    name: String,
    path: String,
}

#[derive(Debug, Deserialize)]
struct SynoResponse {
    success: bool,
    data: Option<serde_json::Value>,
    error: Option<serde_json::Value>,
}

fn list_fileshares(client: &Client, api: &[ApiInfo]) -> Result<Vec<SharedFolder>> {
    let api_name = "SYNO.FileStation.List";
    let version = 2;
    let method = "list_share";
    let api = api.iter().find(|x| x.name == api_name).unwrap();
    assert!(api.name == api_name);
    assert!(version <= api.max_version);
    assert!(api.min_version <= version);

    let resp = client
        .get(&api.path)
        .query(&[
            ("api", api_name),
            ("version", &version.to_string()),
            ("method", method),
        ])
        .send()
        .unwrap()
        .json::<SynoResponse>()
        .unwrap();
    if resp.success {
        let data = resp.data.unwrap();
        let data = data
            .as_object()
            .unwrap()
            .get("shares")
            .unwrap()
            .as_array()
            .unwrap();
        let shares = data
            .iter()
            .map(|x| SharedFolder {
                name: x.get("name").unwrap().as_str().unwrap().to_string(),
                path: x.get("path").unwrap().as_str().unwrap().to_string(),
            })
            .collect::<Vec<SharedFolder>>();
        Ok(shares)
    } else {
        Err(format_error_response(api_name, resp))
    }
}

fn add_dt_to_filename(filename: &std::path::Path) -> String {
    let dt = &chrono::Utc::now().format("%Y%m%d_%H%M%S").to_string();
    let stem = filename
        .file_stem()
        .unwrap()
        .to_str()
        .expect("The file name is pathologic. No stem!");
    let ext = filename.extension().and_then(|x| x.to_str());
    match ext {
        Some(ext) => format!("{stem}_{dt}.{ext}"),
        None => format!("{stem}_{dt}"),
    }
}

fn upload_file(client: &Client, apis: &[ApiInfo], target_path: &str, filename: &str) -> Result<()> {
    let api_name = "SYNO.FileStation.Upload";
    let version = 2;

    let api = apis.iter().find(|x| x.name == api_name).unwrap();
    assert!(version <= api.max_version);
    assert!(api.min_version <= version);

    let filename_path = std::path::PathBuf::from(filename);
    if !filename_path.exists() {
        return Err(anyhow!("File to backup does not exist"));
    }
    let target_file_name = add_dt_to_filename(&filename_path);
    eprintln!(
        "Uploading file {} to {}/{}",
        filename_path.display(),
        target_path,
        target_file_name
    );

    let form = Form::new()
        .text("api", api_name)
        .text("version", version.to_string())
        .text("method", "upload")
        .text("path", target_path.to_string())
        .text("create_parents", "true")
        .text("overwrite", "true")
        .part(
            "file",
            Part::file(filename_path)
                .unwrap()
                .file_name(target_file_name),
        );

    let resp = client
        .post(&api.path)
        .multipart(form)
        .send()
        .unwrap()
        .json::<SynoResponse>()
        .unwrap();
    if resp.success {
        Ok(())
    } else {
        Err(format_error_response(api_name, resp))
    }
}

/// Compresses the contents of a directory into a zip file
/// If the input path is a file, it will be compressed into a zip file
fn compress_iter(
    input_path: &std::path::Path,
    output_path: &std::path::Path,
) -> Result<(), Box<dyn Error>> {
    let inner = File::create(output_path)?;
    let mut zip = ZipWriter::new(inner);
    let options = SimpleFileOptions::default();

    walkdir::WalkDir::new(input_path)
        .into_iter()
        .filter_map(|e| e.ok())
        .filter(|e| e.file_type().is_file())
        .for_each(|input_path| {
            let input_path = input_path.path();
            let mut input_file = File::open(input_path).unwrap();
            let mut buff = Vec::new();
            zip.start_file_from_path(input_path, options).unwrap();
            input_file.read_to_end(&mut buff).unwrap();
            zip.write_all(&buff).unwrap();
        });

    zip.finish()?;
    Ok(())
}

#[derive(Debug, Deserialize)]
struct Config {
    domain: String,
    port: u16,
    share_name: String,
    usr: String,
    pwd: String,
    filename: String,
}

fn main() {
    let config = serde_json::from_str::<Config>(
        &std::fs::read_to_string("config.json").expect("Could not read config file"),
    )
    .expect("Could not parse config file");

    let input_path = config.filename;
    let output_path = input_path.clone() + ".zip";

    compress_iter(
        std::path::Path::new(&input_path),
        std::path::Path::new(&output_path),
    )
    .expect("Failed compressing the target file");

    let client = Client {
        client: reqwest::blocking::Client::builder()
            .cookie_store(true)
            .build()
            .unwrap(),
        base_url: format!("https://{}:{}/webapi", config.domain, config.port),
    };
    let api_info =
        get_api_versions(&client).expect("The API version information could not be retrieved");
    login(&client, &api_info, &config.pwd, &config.usr).expect("Login failed");
    let shares = list_fileshares(&client, &api_info).expect("I should be able to list shares");
    match shares.iter().find(|x| x.name == config.share_name) {
        Some(share) => {
            let share_path = &share.path;
            if let Err(e) = upload_file(&client, &api_info, share_path, &output_path) {
                println!("Error uploading file: {}", e);
            }
        }
        None => {
            println!("Share not found - could not upload file");
        }
    }
    logout(&client, &api_info).unwrap();
}
