# markcollab-backend-core

A WebSocket server written in Rust, handling users and grouping them in "Rooms" (broadcast channels).
This is the core microservice in the MarkCollab project, accepting changes to documents, and broadcasting them to the channel the sender is currently in.

## Communication

### Message format

The server accepts WebSocket messages, with a Json body.

Command message structure:

```json
{
    "command": {
        "type":
            "get-users" | // list users in current room
            "get-user-info" | // get details about thespecified user
            "get-document-metadata" | // get the metadata of the document assigned to this room
            "get-room-metadata" // get the metadata of the current room
        "target": "string" // the target of the command (optional)
    }
}
```

Update message structure:

```json
{
    "update": {
        "type":
            "insert" |
            "delete" |
            "Update",
        "line": "number",
        "column": "number",
        "character": "string" // new character (optional)
    }
}
```
