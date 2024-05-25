use viz::{Request, Response, ResponseExt};

pub async fn index(_r: Request) -> viz::Result<Response> {
    Ok(Response::html(r"¯\_(ツ)_/¯".to_owned()))
}
