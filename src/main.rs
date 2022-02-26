use once_cell::sync::OnceCell;
use rand::prelude::*;
use serde::{Deserialize, Serialize};
use sqlx::PgPool;
use warp::{
    hyper::StatusCode,
    reject::{self, Reject},
    reply, Filter, Rejection,
};

static POOL: OnceCell<PgPool> = OnceCell::new();

#[derive(Deserialize, Serialize)]
struct Deathcard {
    pub owner_id: i32,
    pub card_id: i32,
    pub card_name: String,
    pub attack: i32,
    pub health: i32,
    pub abilities: Vec<String>,
    pub special_abilities: Vec<String>,
    pub stat_icon: String,
    pub blood_cost: i32,
    pub bone_cost: i32,
    pub gem_cost: Vec<i32>,
}

#[derive(Deserialize, Serialize)]
struct User {
    pub user_id: i32,
    pub secret_key: i64,
}

#[derive(Debug)]
enum DeathcardError {
    SqlError,
    NotFound,
    NotAuthorized,
}
impl Reject for DeathcardError {}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    start_postgres().await?;

    let all_deathcards = warp::path!("deathcards")
        .and(warp::get())
        .and_then(|| async move {
            let res: Result<_, Rejection> = Ok(reply::json(&get_all_deathcards().await?));
            res
        });

    let random_deathcards = warp::path!("deathcards" / "random" / usize)
        .and(warp::get())
        .and_then(|count| async move {
            let res: Result<_, Rejection> = Ok(reply::json(&get_random_deathcards(count).await?));
            res
        });

    let add_deathcard = warp::path!("deathcards")
        .and(warp::post())
        .and(warp::body::content_length_limit(4096))
        .and(warp::body::json::<Deathcard>())
        .and_then(|new_card| async move {
            let res: Result<_, Rejection> = Ok(reply::with_status(
                reply::json(&add_deathcard(new_card).await?),
                StatusCode::CREATED,
            ));
            res
        });

    let add_user = warp::path!("user")
        .and(warp::post())
        .and_then(|| async move {
            let res: Result<_, Rejection> = Ok(reply::with_status(
                reply::json(&create_new_user().await?),
                StatusCode::CREATED,
            ));
            res
        });

    let delete_user = warp::path!("users")
        .and(warp::delete())
        .and(warp::body::json::<User>())
        .and_then(|user| async move {
            delete_user(user).await?;
            let res: Result<_, Rejection> = Ok(reply());
            res
        });

    let all = all_deathcards
        .or(random_deathcards)
        .or(add_deathcard)
        .or(add_user)
        .or(delete_user)
        .recover(|e: Rejection| async move {
            if let Some(inner) = e.find::<DeathcardError>() {
                match *inner {
                    DeathcardError::SqlError => Ok(reply::with_status(
                        "SQL Error",
                        StatusCode::INTERNAL_SERVER_ERROR,
                    )),
                    DeathcardError::NotFound => {
                        Ok(reply::with_status("Not Found", StatusCode::NOT_FOUND))
                    }
                    DeathcardError::NotAuthorized => Ok(reply::with_status(
                        "Not Authorized",
                        StatusCode::UNAUTHORIZED,
                    )),
                }
            } else {
                Err(reject::reject())
            }
        });

    warp::serve(all).run(([127, 0, 0, 1], 3030)).await;

    Ok(())
}

async fn get_all_deathcards() -> Result<Vec<Deathcard>, DeathcardError> {
    let pool = POOL.get().unwrap();

    sqlx::query_as!(Deathcard, "SELECT * FROM deathcards")
        .fetch_all(pool)
        .await
        .map_err(|_| DeathcardError::SqlError)
}

async fn get_random_deathcards(count: usize) -> Result<Vec<Deathcard>, DeathcardError> {
    let pool = POOL.get().unwrap();

    let ids: Vec<i32> = sqlx::query!("SELECT card_id FROM deathcards")
        .fetch_all(pool)
        .await
        .map_err(|_| DeathcardError::SqlError)?
        .iter()
        .map(|x| x.card_id)
        .collect();

    let max = count.max(ids.len());

    let mut rng = rand::rngs::StdRng::from_entropy();

    let choices: Vec<i32> = ids.choose_multiple(&mut rng, max).copied().collect();

    sqlx::query_as!(
        Deathcard,
        "SELECT * FROM deathcards WHERE card_id = ANY($1)",
        &choices[..]
    )
    .fetch_all(pool)
    .await
    .map_err(|_| DeathcardError::SqlError)
}

async fn add_deathcard(new: Deathcard) -> Result<Deathcard, DeathcardError> {
    let pool = POOL.get().unwrap();

    sqlx::query!(r#"
        INSERT INTO deathcards 
        (owner_id, card_name, attack, health, abilities, special_abilities, stat_icon, blood_cost, bone_cost, gem_cost) 
        VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10)
        RETURNING card_id
        "#, 
        new.owner_id,
        new.card_name,
        new.attack,
        new.health,
        &new.abilities[..],
        &new.special_abilities[..],
        new.stat_icon,
        new.blood_cost,
        new.bone_cost,
        &new.gem_cost[..]
    )
        .fetch_one(pool)
        .await
        .map(|record| Deathcard { card_id: record.card_id, ..new })
        .map_err(|_| DeathcardError::SqlError)
}

async fn create_new_user() -> Result<User, DeathcardError> {
    let pool = POOL.get().unwrap();

    let secret_key: i64 = StdRng::from_entropy().gen();

    let user_id: i32 = sqlx::query!(
        "INSERT INTO users (secret_key) VALUES ($1) RETURNING user_id",
        secret_key
    )
    .fetch_one(pool)
    .await
    .map_err(|_| DeathcardError::SqlError)?
    .user_id;

    Ok(User {
        user_id,
        secret_key,
    })
}

async fn delete_user(user: User) -> Result<(), DeathcardError> {
    let pool = POOL.get().unwrap();

    let db_user = sqlx::query_as!(
        User,
        "SELECT user_id, secret_key FROM users WHERE user_id = $1",
        user.user_id
    )
    .fetch_optional(pool)
    .await
    .map_err(|_| DeathcardError::SqlError)?
    .ok_or(DeathcardError::NotFound)?;

    if db_user.secret_key != user.secret_key {
        return Err(DeathcardError::NotAuthorized);
    }

    sqlx::query!("DELETE FROM users WHERE user_id = $1", user.user_id)
        .execute(pool)
        .await
        .map_err(|_| DeathcardError::SqlError)?;

    Ok(())
}

async fn start_postgres() -> Result<(), sqlx::Error> {
    POOL.set(PgPool::connect("postgres://localhost/deathcards").await?)
        .unwrap();

    let pool = POOL.get().unwrap();

    sqlx::migrate!().run(pool).await?;

    Ok(())
}
