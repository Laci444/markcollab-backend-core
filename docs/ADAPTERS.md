# Adapter Rétegek Dokumentáció

## Áttekintés

Az `adapters.rs` modul két rétegű adapter architektúrát implementál a WebSocket és Y.js CRDT protokoll közötti kommunikációhoz.

## Réteg Architektúra

```
WebSocket (Axum)
       ↓
  AxumSink/AxumStream       ← WebSocket ↔ Vec<u8>
       ↓
  ProtocolSink/ProtocolStream  ← Vec<u8> ↔ SyncMessage
       ↓
  BroadcastGroup (Y.js CRDT)
```

## 1. WebSocket Adapter Réteg

### AxumSink
Konvertálja a `Vec<u8>` típust `axum::extract::ws::Message::Binary` típusra.

**Trait implementáció:** `Sink<Vec<u8>>`

**Működés:**
```rust
let (sink, stream) = websocket.split();
let axum_sink = AxumSink { inner: sink };

// Most már Vec<u8>-ot lehet küldeni:
axum_sink.send(vec![1, 2, 3, 4]).await?;
```

### AxumStream
Konvertálja az `axum::extract::ws::Message` típust `Vec<u8>` típusra.

**Trait implementáció:** `Stream<Item = Result<Vec<u8>, axum::Error>>`

**Működés:**
- `Message::Binary(data)` → `Ok(data)`
- `Message::Text(text)` → `Ok(text.into_bytes())`
- `Message::Close(_)` → `None` (stream vége)
- `Message::Ping/Pong` → `Pending` (ignorálva)

## 2. Protokoll Adapter Réteg

### ProtocolSink<S>
Konvertálja a `SyncMessage` típust `Vec<u8>` típusra szerializálással.

**Trait implementáció:** `Sink<SyncMessage>`

**Működés:**
```rust
let byte_sink = AxumSink { inner: ws_sink };
let protocol_sink = ProtocolSink::new(byte_sink);

// Most már SyncMessage-t lehet küldeni:
protocol_sink.send(sync_message).await?;
```

**Belső folyamat:**
1. `SyncMessage` → `Vec<u8>` (használja a `write_sync_message` függvényt)
2. Továbbítja az alsó rétegnek (pl. `AxumSink`)

### ProtocolStream<S>
Konvertálja a `Vec<u8>` típust `SyncMessage` típusra parse-olással.

**Trait implementáció:** `Stream<Item = Result<SyncMessage, ProtocolError<E>>>`

**Működés:**
```rust
let byte_stream = AxumStream { inner: ws_stream };
let protocol_stream = ProtocolStream::new(byte_stream);

// Most már SyncMessage-ket kapunk:
while let Some(result) = protocol_stream.next().await {
    match result {
        Ok(sync_msg) => println!("Received: {:?}", sync_msg),
        Err(e) => eprintln!("Error: {}", e),
    }
}
```

**Belső működés:**
1. **Buffer kezelés:** A `ProtocolStream` belső `Vec<u8>` buffert tart
2. **Fragmentált üzenetek:** Ha egy WebSocket frame nem tartalmaz teljes üzenetet:
   - Az új byte-ok hozzáadódnak a bufferhez
   - Parse kísérlet a teljes bufferen
   - Ha `nom::Err::Incomplete`, akkor `Poll::Pending` (vár több adatra)
3. **Sikeres parse:** Az elfogyasztott byte-ok törlődnek a bufferből
4. **Hiba esetén:** A buffer törlődik, `ProtocolError::Decode` visszaadva

## Használati Példák

### Alapvető használat (jelenlegi implementáció)

```rust
async fn handle_websocket(ws: WebSocket, bcast: Arc<BroadcastGroup>) {
    let (sink, stream) = ws.split();
    
    // Csak WebSocket adapter réteg
    let axum_sink = AxumSink { inner: sink };
    let axum_stream = AxumStream { inner: stream };
    
    // BroadcastGroup belül kezeli a protokollt
    let subscription = bcast.subscribe(axum_sink, axum_stream);
    subscription.completed().await;
}
```

### Protokoll réteg használata (opcionális)

```rust
async fn handle_websocket_with_protocol(ws: WebSocket, bcast: Arc<BroadcastGroup>) {
    let (sink, stream) = ws.split();
    
    // 1. WebSocket adapter
    let byte_sink = AxumSink { inner: sink };
    let byte_stream = AxumStream { inner: stream };
    
    // 2. Protokoll adapter (kompozíció)
    let protocol_sink = ProtocolSink::new(byte_sink);
    let protocol_stream = ProtocolStream::new(byte_stream);
    
    // Most már SyncMessage-ekkel dolgozunk
    // TODO: Implementálni kell a BroadcastGroup::subscribe_protocol metódust
    let subscription = bcast.subscribe_protocol(protocol_sink, protocol_stream);
    subscription.completed().await;
}
```

## Error Handling

### ProtocolError<E>
Két típusú hibát kezel:

1. **Transport(E):** Az alsó réteg (WebSocket) hibája
   - Példa: Hálózati kapcsolat megszakadás
   
2. **Decode(JwstCodecError):** Protokoll parse hiba
   - Példa: Érvénytelen üzenet formátum

```rust
match protocol_stream.next().await {
    Some(Ok(msg)) => { /* Sikeres üzenet */ },
    Some(Err(ProtocolError::Transport(e))) => {
        eprintln!("Hálózati hiba: {}", e);
    },
    Some(Err(ProtocolError::Decode(e))) => {
        eprintln!("Protokoll hiba: {}", e);
    },
    None => { /* Stream vége */ },
}
```

## Performance Jellemzők

### Zero-Cost Abstraction
- Az adapterek csak `#[repr(transparent)]` wrapper struct-ok
- A compiler inline-olja a függvényhívásokat
- Nincs heap allokáció az adapter rétegekben (csak a buffer a `ProtocolStream`-ben)

### Buffer Stratégia
- **ProtocolStream buffer:** Dinamikusan nő, ahogy fragmentált adatok érkeznek
- **Memória felszabadítás:** Sikeres parse után a feldolgozott byte-ok azonnal törlődnek
- **Hiba esetén:** A teljes buffer törlődik, megelőzve a memória szivárgást

### Backpressure Kezelés
- A `poll_ready` és `poll_flush` metódusok propagálják az alsó réteg backpressure jelzéseit
- Ha az alsó réteg `Poll::Pending`-et ad vissza, a felső réteg is várakozik

## Tesztelés

A build tesztelése:
```bash
cargo check
cargo build --release
```

Runtime teszt mock auth-tal:
```bash
USE_MOCK_AUTH=1 cargo run
# Másik terminálban:
websocat "ws://localhost:3030/ws/test-room?username=alice"
```

## Jövőbeli Fejlesztések

1. **BroadcastGroup::subscribe_protocol()** implementálása
2. **Metrikák:** Üzenet számlálók, buffer méret monitoring
3. **Konfiguráció:** Maximum buffer méret beállítása
4. **Compression:** Opcionális tömörítés a protokoll rétegben
5. **Multiplexing:** Több stream kezelése egy WebSocket kapcsolaton
