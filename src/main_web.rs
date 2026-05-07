use axum::{routing::get, Json, Router};
use rusqlite::{params, Connection};
use serde::Serialize;
use std::net::SocketAddr;

#[derive(Serialize)]
struct Mover {
    category: String,
    gain: i32,
    rank: i32,
    price: String,
    title: String,
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // 1. On lance le serveur web dans un thread séparé
    tokio::spawn(async {
        let app = Router::new().route("/api/movers", get(get_movers));
        let addr = SocketAddr::from(([127, 0, 0, 1], 3000));
        println!("🌐 Dashboard API dispo sur http://{} ", addr);
        axum::Server::bind(&addr).serve(app.into_make_service()).await.unwrap();
    });

    // 2. Ton moteur de scraping (on garde la même logique qu avant)
    // Ici tu peux remettre ton code de scraping dans une boucle loop {}
    // pour qu il tourne toutes les X heures automatiquement.
    
    println!("🚀 Moteur de surveillance actif. Appuie sur Ctrl+C pour stopper.");
    
    // Garde le main en vie
    loop {
        tokio::time::sleep(tokio::time::Duration::from_secs(3600)).await;
        // Optionnel : relancer un scan ici
    }
}

async fn get_movers() -> Json<Vec<Mover>> {
    let conn = Connection::open("trending_products.db").unwrap();
    let mut stmt = conn.prepare("SELECT category, rank_gain, current_rank, price, title FROM top_movers WHERE rank_gain > 5 LIMIT 50").unwrap();
    
    let movers = stmt.query_map([], |row| {
        Ok(Mover {
            category: row.get(0)?,
            gain: row.get(1)?,
            rank: row.get(2)?,
            price: row.get::<_, Option<String>>(3)?.unwrap_or_else(|| "N/A".to_string()),
            title: row.get(4)?,
        })
    }).unwrap().map(|r| r.unwrap()).collect();

    Json(movers)
}

