use reqwest::header::{USER_AGENT, ACCEPT, ACCEPT_LANGUAGE};
use scraper::{Html, Selector};
use rusqlite::{params, Connection};
use chrono::Utc;
use tokio::task;

#[derive(Debug, Clone)]
struct Product {
    rank: i32,
    title: String,
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let db_path = "db";
    setup_db(db_path)?;

    let categories = vec![
        ("Electronics", "https://www.amazon.fr/gp/movers-and-shakers/electronics"),
        ("Kitchen", "https://www.amazon.fr/gp/movers-and-shakers/kitchen"),
        ("Home", "https://www.amazon.fr/gp/movers-and-shakers/home"),
        ("Video Games", "https://www.amazon.fr/gp/movers-and-shakers/videogames"),
        ("Toys", "https://www.amazon.fr/gp/movers-and-shakers/toys"),
        ("Tools", "https://www.amazon.fr/gp/movers-and-shakers/hi"),
    ];

    println!("🚀 Moteur lancé sur {} threads virtuels...", categories.len());

    let mut handles = vec![];

    for (name, url) in categories {
        let name = name.to_string();
        let url = url.to_string();
        
        let handle = task::spawn(async move {
            match scrape_category(&url).await {
                Ok(products) => Some((name, products)),
                Err(e) => {
                    eprintln!("❌ Erreur {}: {}", name, e);
                    None
                }
            }
        });
        handles.push(handle);
    }

    let mut conn = Connection::open(db_path)?;

    for handle in handles {
        if let Some((cat_name, products)) = handle.await? {
            println!("\n📊 Résultats pour : {}", cat_name);
            let tx = conn.transaction()?;
            
            for p in products {
                // 1. Chercher l ancien rang pour calculer le momentum
                let prev_rank: Option<i32> = tx.query_row(
                    "SELECT rank FROM product_trends WHERE title = ?1 AND category = ?2 ORDER BY captured_at DESC LIMIT 1",
                    params![p.title, cat_name],
                    |row| row.get(0),
                ).ok();

                let momentum_str = match prev_rank {
                    Some(old) if old > p.rank => format!("🔥 ↑ +{}", old - p.rank),
                    Some(old) if old < p.rank => format!("🧊 ↓ -{}", p.rank - old),
                    Some(_) => "➡️ Stable".to_string(),
                    None => "✨ Nouveau !".to_string(),
                };

                println!("{:02} | {} | {}", p.rank, momentum_str, p.title);

                // 2. Sauvegarder le nouveau rang
                tx.execute(
                    "INSERT INTO product_trends (category, rank, title, captured_at) VALUES (?1, ?2, ?3, ?4)",
                    params![cat_name, p.rank, p.title, Utc::now().to_rfc3339()],
                )?;
            }
            tx.commit()?;
        }
    }

    Ok(())
}

fn setup_db(path: &str) -> rusqlite::Result<()> {
    let conn = Connection::open(path)?;
    conn.execute(
        "CREATE TABLE IF NOT EXISTS product_trends (
            id INTEGER PRIMARY KEY,
            category TEXT,
            rank INTEGER,
            title TEXT,
            captured_at DATETIME
        )",
        [],
    )?;
    // Index pour accélérer les recherches de momentum
    conn.execute("CREATE INDEX IF NOT EXISTS idx_search ON product_trends (title, category)", [])?;
    Ok(())
}

async fn scrape_category(url: &str) -> Result<Vec<Product>, Box<dyn std::error::Error + Send + Sync>> {
    let client = reqwest::Client::new();
    let response = client
        .get(url)
        .header(USER_AGENT, "Mozilla/5.0 (X11; Linux x86_64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/124.0.0.0 Safari/537.36")
        .header(ACCEPT, "text/html,application/xhtml+xml,application/xml;q=0.9,image/avif,image/webp,*/*;q=0.8")
        .header(ACCEPT_LANGUAGE, "fr,fr-FR;q=0.8,en-US;q=0.5,en;q=0.3")
        .send()
        .await?
        .text()
        .await?;

    let document = Html::parse_document(&response);
    let img_selector = Selector::parse("img.p13n-product-image, img[alt]").unwrap();
    
    let mut products = Vec::new();
    let mut rank = 1;

    for element in document.select(&img_selector) {
        if let Some(alt) = element.value().attr("alt") {
            if alt.len() > 15 && !alt.contains("arrow") {
                products.push(Product { rank, title: alt.trim().to_string() });
                rank += 1;
            }
        }
    }
    Ok(products)
}
