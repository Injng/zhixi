use rocket::request::{Outcome, Request, FromRequest};
use rocket::http::Status;

#[derive(Debug, Clone)]
pub struct AuthUser {
    pub id: i64,
}

#[rocket::async_trait]
impl<'r> FromRequest<'r> for AuthUser {
    type Error = ();

    async fn from_request(request: &'r Request<'_>) -> Outcome<Self, Self::Error> {
        match request.cookies().get_private("user_id") {
            Some(cookie) => {
                match cookie.value().parse::<i64>() {
                    Ok(id) => Outcome::Success(AuthUser { id }),
                    Err(_) => Outcome::Forward(Status::Unauthorized),
                }
            },
            None => Outcome::Forward(Status::Unauthorized),
        }
    }
}

