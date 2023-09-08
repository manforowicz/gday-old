# Message Format

## Header

2 bytes - total inclusive length of the entire message

1 byte - type of the message (see below)

### 1 - client: "create room" (transmits new room password)

password: array of bytes

### 2 - client: "join room" (transmits password of existing room)

password: array of bytes

### 3 - server: "room joined" (transmits new user id)

user id: 8 bytes

### 4 - client: "sending contact info"

user id: 8 bytes

private ipv6 (only sent if known)
private ipv4 (only sent if known)

"5" byte - at end if nothing else to share / request peer info when available

### 5 - server: "sharing peer info"

series of optional: flag byte followed by content

    "1" byte - a peer's contact info comes next

    "2" byte - private ips are next
    private ipv6 (only sent if known)
    private ipv4 (only sent if known)

    "3" byte - public ip's are next
    public ipv4 (only sent if known)
    public ipv6 (only sent if known)




# ERROR MESSAGE TYPES (no content in them)


### 6 - invalid message syntax
### 7 - room with this password already exists
### 8 - no room with this password exists
### 9 - unknown personal id
### 255 - other

# IP ADDRESS FORMAT

- ipv4 - "4" (1 byte), ip (4 bytes),  port (2 bytes)
- ipv6 - "6" (1 byte), ip (16 bytes), port (2 bytes)
