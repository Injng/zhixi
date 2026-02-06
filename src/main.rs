#[macro_use] extern crate rocket;

mod db;
mod models;
mod routes;
mod auth;
mod translate;

use rocket_db_pools::Database;
use db::Db;

use rocket::fairing::AdHoc;

use rocket::fs::FileServer;

#[launch]
fn rocket() -> _ {
    rocket::build()
        .attach(Db::init())
        .attach(AdHoc::try_on_ignite("SQLx Migrations", |rocket| async {
            let db = Db::fetch(&rocket).expect("database connection");
            match sqlx::migrate!().run(&**db).await {
                Ok(_) => Ok(rocket),
                Err(e) => {
                    eprintln!("Failed to initialize SQLx migrations: {}", e);
                    Err(rocket)
                }
            }
        }))
        .mount("/", routes::routes())
        .mount("/uploads", FileServer::from("uploads"))
}
