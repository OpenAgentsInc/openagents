//! Fixed-price CAD purchases paid directly to the designer over Lightning.
use super::*;
use std::time::{SystemTime, UNIX_EPOCH};
mod lightning;
mod web;
pub(super) fn routes() -> Router<App> {
    web::routes()
}
const CATALOG: &str = "cad-shop/index.json";
const FILES: [(&str, &str); 4] = [
    ("step", "design.step"),
    ("stl", "design.stl"),
    ("pdf", "drawing.pdf"),
    ("dxf", "drawing.dxf"),
];
fn now() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs()
}
fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}
fn unhex(s: &str) -> Option<Vec<u8>> {
    if s.len() % 2 != 0 {
        return None;
    }
    s.as_bytes()
        .chunks_exact(2)
        .map(|p| u8::from_str_radix(std::str::from_utf8(p).ok()?, 16).ok())
        .collect()
}
fn token() -> String {
    hex(&secp256k1::rand::random::<[u8; 24]>())
}
fn valid(s: &str) -> bool {
    s.len() == 48
        && s.bytes()
            .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
}
fn listing_key(id: &str) -> String {
    format!("cad-products/{id}/listing.json")
}
fn order_key(id: &str) -> String {
    format!("cad-orders/{id}/order.json")
}
fn package_key(id: &str) -> String {
    format!("cad-orders/{id}/files.zip")
}
#[derive(Clone, Serialize, Deserialize)]
struct Listing {
    home: String,
    folder: String,
    project: Option<String>,
    doc: String,
    data: Value,
    title: String,
    sats: u64,
    address: String,
    active: bool,
    created: u64,
}
#[derive(Clone, Serialize, Deserialize)]
struct Brief {
    title: String,
    sats: u64,
    active: bool,
}
#[derive(Clone, Serialize, Deserialize)]
struct Order {
    buyer: String,
    listing: String,
    title: String,
    version: u64,
    labels: Vec<String>,
    values: Vec<f64>,
    sats: u64,
    address: String,
    created: u64,
    invoice: Option<lightning::Invoice>,
    package_hash: Option<String>,
    paid: bool,
    paid_at: Option<u64>,
}
async fn read<T: serde::de::DeserializeOwned>(
    store: &Store,
    key: &str,
) -> Result<Option<(T, String)>, String> {
    store
        .read_key(key)
        .await
        .map_err(|_| "This item couldn't be opened.")?
        .map(|(b, g)| {
            serde_json::from_slice(&b)
                .map(|v| (v, g))
                .map_err(|_| "This saved item couldn't be read.".into())
        })
        .transpose()
}
async fn catalog_change(store: &Store, id: &str, add: bool) -> Result<(), String> {
    for _ in 0..5 {
        let old = read::<Vec<String>>(store, CATALOG).await?;
        let mut ids = old.as_ref().map(|(v, _)| v.clone()).unwrap_or_default();
        ids.retain(|s| s != id);
        if add {
            if ids.len() >= 500 {
                return Err("The shop is full right now. Try listing the design later.".into());
            }
            ids.insert(0, id.into());
        }
        match store
            .write_key(
                CATALOG,
                serde_json::to_vec(&ids).map_err(|_| "The shop couldn't be updated.")?,
                old.as_ref().map(|(_, g)| g.as_str()),
            )
            .await
        {
            Ok(_) => return Ok(()),
            Err(StoreError::Conflict) => continue,
            Err(_) => return Err("The shop couldn't be updated.".into()),
        }
    }
    Err("The shop changed in another window. Try again.".into())
}
pub(super) async fn listings(
    store: &Store,
    scope: &Scope,
    doc: &str,
) -> Result<Vec<(String, String, u64)>, String> {
    let prefix = scope
        .key(&format!("{doc}/products"))
        .map_err(|_| "This design couldn't be opened.")?;
    let keys = store
        .folder_keys(&prefix)
        .await
        .map_err(|_| "The listings couldn't be opened.")?;
    let mut items = Vec::new();
    for key in keys {
        if let Some(id) = key
            .rsplit('/')
            .next()
            .and_then(|s| s.strip_suffix(".json"))
            .filter(|s| valid(s))
        {
            if let Some((listing, _)) = read::<Listing>(store, &listing_key(id)).await? {
                if listing.active {
                    items.push((id.into(), listing.title, listing.sats));
                }
            }
        }
    }
    Ok(items)
}
pub(super) async fn create(
    store: &Store,
    scope: &Scope,
    doc: &str,
    data: Value,
    sats: u64,
    address: &str,
) -> Result<String, String> {
    if !scope.edits || scope.actor != scope.home {
        return Err("Only the design's owner can list it for sale.".into());
    }
    if !(1..=10_000_000).contains(&sats) {
        return Err("Choose a price from 1 to 10,000,000 sats.".into());
    }
    lightning::address(address)?;
    if listings(store, scope, doc).await?.len() >= 20 {
        return Err("This design has 20 listings. Stop one before adding another.".into());
    }
    let id = token();
    let listing = Listing {
        home: scope.home.clone(),
        folder: scope.folder.clone(),
        project: scope.project.clone(),
        doc: doc.into(),
        title: data["title"].as_str().unwrap_or(doc).into(),
        data,
        sats,
        address: address.trim().into(),
        active: true,
        created: now(),
    };
    let bytes = serde_json::to_vec(&listing).map_err(|_| "The listing couldn't be saved.")?;
    if bytes.len() > 8 << 20 {
        return Err("This design is too large for the shop.".into());
    }
    store
        .write_key(&listing_key(&id), bytes, None)
        .await
        .map_err(|_| "The listing couldn't be saved.")?;
    let brief = Brief {
        title: listing.title.clone(),
        sats: listing.sats,
        active: true,
    };
    store
        .write_key(
            &format!("cad-products/{id}/public.json"),
            serde_json::to_vec(&brief).map_err(|_| "The listing couldn't be saved.")?,
            None,
        )
        .await
        .map_err(|_| "The listing couldn't be saved.")?;
    let result = async {
        store
            .write_key(
                &scope
                    .key(&format!("{doc}/products/{id}.json"))
                    .map_err(|_| "The listing couldn't be saved.")?,
                b"{}".to_vec(),
                None,
            )
            .await
            .map_err(|_| "The listing couldn't be saved.")?;
        catalog_change(store, &id, true).await
    }
    .await;
    if let Err(error) = result {
        let _ = store.remove_folder(&format!("cad-products/{id}")).await;
        return Err(error);
    }
    Ok(id)
}
async fn stop(store: &Store, scope: &Scope, doc: &str, id: &str) -> Result<(), String> {
    if !valid(id) || !scope.edits || scope.actor != scope.home {
        return Err("Only the design's owner can stop its listing.".into());
    }
    for _ in 0..5 {
        let Some((mut listing, generation)) = read::<Listing>(store, &listing_key(id)).await?
        else {
            return Ok(());
        };
        if listing.home != scope.home || listing.folder != scope.folder || listing.doc != doc {
            return Err("This listing belongs to another design.".into());
        }
        listing.active = false;
        listing.data = Value::Null;
        match store
            .write_key(
                &listing_key(id),
                serde_json::to_vec(&listing).map_err(|_| "The listing couldn't be updated.")?,
                Some(&generation),
            )
            .await
        {
            Ok(_) => {
                put(
                    store,
                    &format!("cad-products/{id}/public.json"),
                    serde_json::to_vec(&Brief {
                        title: listing.title.clone(),
                        sats: listing.sats,
                        active: false,
                    })
                    .map_err(|_| "The listing couldn't be updated.")?,
                )
                .await?;
                catalog_change(store, id, false).await?;
                return Ok(());
            }
            Err(StoreError::Conflict) => continue,
            Err(_) => return Err("The listing couldn't be updated.".into()),
        }
    }
    Err("This listing changed. Reload and try again.".into())
}
pub(super) async fn purge(store: &Store, scope: &Scope, doc: &str) -> Result<(), String> {
    if !scope.edits {
        return Err("Only the project's editors can delete this design.".into());
    }
    let mut owner = scope.clone();
    owner.actor = owner.home.clone();
    for (id, _, _) in listings(store, scope, doc).await? {
        stop(store, &owner, doc, &id).await?;
    }
    Ok(())
}

impl Listing {
    fn scope(&self, buyer: &str) -> Scope {
        Scope {
            home: self.home.clone(),
            folder: self.folder.clone(),
            project: self.project.clone(),
            edits: false,
            owns: false,
            actor: buyer.into(),
        }
    }
    fn values(&self, values: &[f64]) -> Result<(), String> {
        let controls = self.data["controls"]
            .as_array()
            .ok_or("This listing has no dimensions.")?;
        if values.len() != controls.len()
            || values.len() > 8
            || values.iter().zip(controls).any(|(v, c)| {
                !v.is_finite()
                    || *v < c["min"].as_f64().unwrap_or(f64::INFINITY)
                    || *v > c["max"].as_f64().unwrap_or(f64::NEG_INFINITY)
            })
        {
            return Err("Choose a value within each slider's limits.".into());
        }
        Ok(())
    }
}
async fn prepare(
    store: &Store,
    buyer: &str,
    listing_id: &str,
    id: &str,
    values: Vec<f64>,
) -> Result<Order, String> {
    if !valid(id) || !valid(listing_id) {
        return Err("Reload the listing before buying.".into());
    }
    let Some((listing, _)) = read::<Listing>(store, &listing_key(listing_id))
        .await?
        .filter(|(l, _)| l.active)
    else {
        return Err("This design is no longer for sale.".into());
    };
    listing.values(&values)?;
    let initial = Order {
        buyer: buyer.into(),
        listing: listing_id.into(),
        title: listing.title.clone(),
        version: listing.data["version"]
            .as_u64()
            .ok_or("This listing has no design version.")?,
        labels: listing.data["controls"]
            .as_array()
            .unwrap()
            .iter()
            .map(|c| c["label"].as_str().unwrap_or("Dimension").into())
            .collect(),
        values: values.clone(),
        sats: listing.sats,
        address: listing.address.clone(),
        created: now(),
        invoice: None,
        package_hash: None,
        paid: false,
        paid_at: None,
    };
    let key = order_key(id);
    match store
        .write_key(
            &key,
            serde_json::to_vec(&initial).map_err(|_| "The order couldn't be saved.")?,
            None,
        )
        .await
    {
        Ok(_) | Err(StoreError::Conflict) => {}
        Err(_) => return Err("The order couldn't be saved.".into()),
    }
    let (mut order, generation) = read::<Order>(store, &key)
        .await?
        .ok_or("The order couldn't be opened.")?;
    if order.buyer != buyer || order.listing != listing_id || order.values != values {
        return Err("This order belongs to another purchase. Reload the listing.".into());
    }
    let account = Scope::personal(buyer).ok_or("Log in to buy this design.")?;
    let index_key = account
        .key(&format!("purchases/{id}.json"))
        .map_err(|_| "The order couldn't be saved.")?;
    match store.write_key(&index_key, b"{}".to_vec(), None).await {
        Ok(_) | Err(StoreError::Conflict) => {}
        Err(_) => return Err("The order couldn't be saved.".into()),
    }
    if order.invoice.is_some() {
        return Ok(order);
    }
    let engine = engine().ok_or("The CAD engine isn't available. Try again later.")?;
    let result=engine.run(store,&listing.scope(buyer),"cad_product",&json!({"doc":listing.doc,"version":order.version,"controls":listing.data["controls"],"values":order.values,"order":id})).await;
    if let Some(error) = result["error"].as_str() {
        return Err(error.into());
    }
    let file = result["file"]
        .as_str()
        .filter(|f| valid_out(f))
        .ok_or("The design files weren't prepared.")?;
    let bytes = read_file(
        store,
        &listing.scope(buyer),
        &listing.doc,
        order.version,
        file,
    )
    .await
    .ok_or("The design files weren't saved. Try again.")?;
    let package = package_key(id);
    match store.write_key(&package, bytes, None).await {
        Ok(_) | Err(StoreError::Conflict) => {}
        Err(_) => return Err("The design files couldn't be saved.".into()),
    }
    let bytes = store
        .read_key(&package)
        .await
        .map_err(|_| "The design files couldn't be read.")?
        .ok_or("The design files couldn't be read.")?
        .0;
    order.package_hash = Some(hex(&Sha256::digest(&bytes)));
    let invoice = lightning::create(&order.address, order.sats, now()).await?;
    let reservation = format!("cad-payment-hashes/{}.json", invoice.hash);
    match store
        .write_key(&reservation, id.as_bytes().to_vec(), None)
        .await
    {
        Ok(_) => {}
        Err(StoreError::Conflict) => {
            if store
                .read_key(&reservation)
                .await
                .map_err(|_| "The invoice couldn't be checked.")?
                .map(|(b, _)| b)
                != Some(id.as_bytes().to_vec())
            {
                return Err(
                    "The wallet reused an invoice. No payment was requested; try again.".into(),
                );
            }
        }
        Err(_) => return Err("The invoice couldn't be saved.".into()),
    }
    order.invoice = Some(invoice);
    match store
        .write_key(
            &key,
            serde_json::to_vec(&order).map_err(|_| "The order couldn't be saved.")?,
            Some(&generation),
        )
        .await
    {
        Ok(_) => Ok(order),
        Err(StoreError::Conflict) => read::<Order>(store, &key)
            .await?
            .map(|(o, _)| o)
            .filter(|o| o.buyer == buyer && o.invoice.is_some())
            .ok_or("This order changed. Open it again.".into()),
        Err(_) => Err("The order couldn't be saved.".into()),
    }
}
async fn pay_check(store: &Store, buyer: &str, id: &str) -> Result<bool, String> {
    for _ in 0..5 {
        let Some((mut order, generation)) = read::<Order>(store, &order_key(id))
            .await?
            .filter(|(o, _)| o.buyer == buyer)
        else {
            return Err("This order couldn't be found.".into());
        };
        if order.paid {
            return Ok(true);
        }
        let invoice = order
            .invoice
            .as_ref()
            .ok_or("Continue checkout before checking payment.")?;
        if !lightning::settled(invoice).await? {
            return Ok(false);
        }
        if store
            .read_key(&format!("cad-payment-hashes/{}.json", invoice.hash))
            .await
            .map_err(|_| "The payment couldn't be checked.")?
            .map(|(b, _)| b)
            != Some(id.as_bytes().to_vec())
        {
            return Err("This invoice belongs to another order.".into());
        }
        order.paid = true;
        order.paid_at = Some(now());
        match store
            .write_key(
                &order_key(id),
                serde_json::to_vec(&order).map_err(|_| "The payment couldn't be saved.")?,
                Some(&generation),
            )
            .await
        {
            Ok(_) => return Ok(true),
            Err(StoreError::Conflict) => continue,
            Err(_) => {
                return Err(
                    "The payment couldn't be saved. Check again; keep the same invoice.".into(),
                );
            }
        }
    }
    Err("This order changed. Open it again.".into())
}

#[cfg(test)]
pub(super) mod tests;
