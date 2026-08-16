# MAVLink Protocol Handbook

A condensed engineering reference to the MAVLink protocol, distilled from the
official documentation at <https://mavlink.io/en/>. It is written for JFOXLink
developers who need to understand the protocol JFOXLink is derived from and
wraps (see [Part 9](#part-9--mavlink-and-jfoxlink)).

This handbook is a study/reference document, not a normative specification.
Where a detail matters for interoperability, the authoritative source is
`mavlink/message_definitions/v1.0/*.xml` and the pages linked in each section.

---

## Contents

- [Part 1 — Overview](#part-1--overview)
- [Part 2 — Packet Serialization](#part-2--packet-serialization)
- [Part 3 — Checksums](#part-3--checksums)
- [Part 4 — Message Signing (Authentication)](#part-4--message-signing-authentication)
- [Part 5 — Versions and Negotiation](#part-5--versions-and-negotiation)
- [Part 6 — Routing and Addressing](#part-6--routing-and-addressing)
- [Part 7 — Message Definitions (XML Dialects)](#part-7--message-definitions-xml-dialects)
- [Part 8 — Microservices](#part-8--microservices)
  - [8.1 Heartbeat / Connection](#81-heartbeat--connection-protocol)
  - [8.2 Command](#82-command-protocol)
  - [8.3 Mission](#83-mission-protocol)
  - [8.4 Parameter](#84-parameter-protocol)
  - [8.5 Telemetry / Message Rates](#85-telemetry-and-message-rates)
  - [8.6 Manual Control](#86-manual-control-joystick)
  - [8.7 File Transfer (FTP)](#87-file-transfer-protocol-ftp)
  - [8.8 Camera v2](#88-camera-protocol-v2)
  - [8.9 Gimbal v2](#89-gimbal-protocol-v2)
  - [8.10 Other Microservices](#810-other-microservices)
- [Part 9 — MAVLink and JFOXLink](#part-9--mavlink-and-jfoxlink)
- [Appendix A — Key Messages Reference](#appendix-a--key-messages-reference)
- [Appendix B — Key Enums](#appendix-b--key-enums)
- [Appendix C — Frequently Used MAV_CMD Commands](#appendix-c--frequently-used-mav_cmd-commands)
- [Appendix D — Worked Example: Encoding a HEARTBEAT](#appendix-d--worked-example-encoding-a-heartbeat)
- [Appendix E — Implementation Checklist](#appendix-e--implementation-checklist)
- [Appendix F — Source Pages](#appendix-f--source-pages)

---

## Part 1 — Overview

**MAVLink** (Micro Air Vehicle Link) is a very lightweight, binary messaging
protocol for communicating with drones and between onboard drone components.
It was first released in 2009 and is now governed under the Dronecode
Project (Linux Foundation).

### Design characteristics

| Property | Detail |
|---|---|
| Design pattern | Hybrid: **publish–subscribe** for data streams (telemetry) plus **point-to-point** request/acknowledge sub-protocols ("microservices") for configuration and control |
| Overhead | MAVLink 1: **8 bytes** per packet. MAVLink 2: **12 bytes** per packet (+13 if signed) |
| Reliability | Sequence numbers for loss detection, CRC-16 integrity check, `CRC_EXTRA` definition check, optional SHA-256 signing |
| Scale | Up to **255 systems** on a network, each with up to 255 components |
| Byte order | **Little-endian** for all multi-byte fields |
| Transport | Transport-agnostic: serial/UART, UDP, TCP, USB, radio modems, etc. |
| Portability | Runs on 8-bit MCUs through desktop OSes; generated headers in C are header-only |

### Official language bindings

C, C++11, Python, Rust, JavaScript, TypeScript, Java, C#, Lua, Ada,
Objective-C, Swift (plus community: Go, Kotlin, Dart, Clojure, …).
Both MAVLink 1 and 2 are supported; most bindings also support signing.

### Terminology

| Term | Meaning |
|---|---|
| **System** | A vehicle, GCS, companion computer, … identified by `system_id` (1–255) |
| **Component** | A sub-part of a system (autopilot, camera, gimbal, …) identified by `component_id` (1–255) |
| **Message** | A typed packet defined in XML with a numeric `id` and a fixed field layout |
| **Dialect** | An XML file of message/enum definitions; `common.xml` is the shared standard, vendors add their own (`ardupilotmega.xml`, `development.xml`, …) |
| **Microservice** | A higher-level protocol built out of messages (mission, parameter, command, …) |
| **GCS** | Ground Control Station |

---

## Part 2 — Packet Serialization

Source: <https://mavlink.io/en/guide/serialization.html>

### 2.1 MAVLink 2 packet format

Maximum packet length **280 bytes**; minimum **12 bytes** (empty payload,
unsigned).

| Byte offset | Field | Size | Value / Notes |
|---|---|---|---|
| 0 | `magic` (STX) | 1 | `0xFD` |
| 1 | `len` | 1 | Payload length 0–255 (after truncation, see 2.4) |
| 2 | `incompat_flags` | 1 | Flags that **must** be understood; unknown bits → discard packet. `0x01` = `MAVLINK_IFLAG_SIGNED` |
| 3 | `compat_flags` | 1 | Flags that **may** be ignored if not understood (none defined in `common`) |
| 4 | `seq` | 1 | Sequence counter 0–255, per sender; wraps; used for loss detection |
| 5 | `sysid` | 1 | Sending system ID (1–255; 0 is invalid as source) |
| 6 | `compid` | 1 | Sending component ID (1–255; 0 is invalid as source) |
| 7–9 | `msgid` | 3 | Message ID 0–16 777 215, little-endian (low byte first) |
| 10 … 9+n | `payload` | n | Message fields, reordered (2.3) and truncated (2.4) |
| 10+n, 11+n | `checksum` | 2 | CRC-16/MCRF4XX (X.25) over bytes 1..end-of-payload plus `CRC_EXTRA`, little-endian |
| 12+n … 24+n | `signature` | 13 (optional) | Present iff `incompat_flags & 0x01`; see Part 4 |

### 2.2 MAVLink 1 packet format

Maximum packet length **263 bytes**; minimum **8 bytes**.

| Byte offset | Field | Size | Value / Notes |
|---|---|---|---|
| 0 | `magic` (STX) | 1 | `0xFE` |
| 1 | `len` | 1 | Payload length (fixed per message in v1 — no truncation) |
| 2 | `seq` | 1 | Sequence counter |
| 3 | `sysid` | 1 | Sending system ID |
| 4 | `compid` | 1 | Sending component ID |
| 5 | `msgid` | 1 | Message ID 0–255 |
| 6 … 5+n | `payload` | n | Message fields, reordered |
| 6+n, 7+n | `checksum` | 2 | CRC-16/MCRF4XX + `CRC_EXTRA` |

MAVLink 1 has no flags, no signing, and only 256 message IDs.

### 2.3 Payload field reordering

Fields in the payload are **not** in XML declaration order. The generator
sorts them by native type size, **largest first**, keeping declaration order
among equal sizes:

1. 8-byte: `uint64_t`, `int64_t`, `double`
2. 4-byte: `uint32_t`, `int32_t`, `float`
3. 2-byte: `uint16_t`, `int16_t`
4. 1-byte: `uint8_t`, `int8_t`, `char`

For arrays the *element* size is used. This aligns fields naturally on most
architectures so a payload can be overlaid on a packed C struct.

**Exception — MAVLink 2 extension fields** (those after `<extensions/>` in
XML) are appended **after** the base fields, in **declaration order**,
unsorted, and are **excluded** from `CRC_EXTRA`. This lets old receivers
ignore new fields.

### 2.4 Payload truncation (MAVLink 2 only)

Senders **must** strip trailing zero bytes from the serialized payload
(after reordering), except that the **first byte is never removed** — the
minimum payload length is 1. Receivers must zero-fill the payload back to the
message's full length before decoding, and must also accept non-compliant
senders that leave the zeros in place.

Because of reordering, the smallest fields (and extensions) sit at the end,
so truncation typically removes unused extension fields and zero-valued small
fields.

### 2.5 Sequence numbers

Each sender keeps one 8-bit counter per channel, incremented for every
packet. Receivers detect drops by gaps and can compute per-link loss rate.
Message forwarders/routers must **not** rewrite `seq`, `sysid`, or `compid`.

### 2.6 Constraints

- `sysid = 0` and `compid = 0` are broadcast addresses and are **invalid as
  source addresses**.
- All multi-byte integers and floats are little-endian IEEE-754.
- `char[n]` fields are not required to be NUL-terminated if fully used.

---

## Part 3 — Checksums

### 3.1 CRC-16/MCRF4XX (packet checksum)

Also known as X.25 CRC / CRC-16-CCITT reflected variant.

| Parameter | Value |
|---|---|
| Width | 16 bits |
| Polynomial | `0x1021` (reflected: `0x8408`) |
| Init | `0xFFFF` |
| Reflect in/out | yes |
| Final XOR | none |
| Check (`"123456789"`) | `0x6F91` |

Reference implementation (from `checksum.h`):

```c
static inline void crc_accumulate(uint8_t data, uint16_t *crcAccum)
{
    uint8_t tmp = data ^ (uint8_t)(*crcAccum & 0xff);
    tmp ^= (tmp << 4);
    *crcAccum = (*crcAccum >> 8) ^ (tmp << 8) ^ (tmp << 3) ^ (tmp >> 4);
}
/* init: *crcAccum = 0xFFFF */
```

**Bytes covered:** every byte from `len` (offset 1) through the last payload
byte — i.e. everything **except** the STX byte and the signature — followed by
one extra byte, `CRC_EXTRA`.

### 3.2 CRC_EXTRA (definition checksum)

`CRC_EXTRA` is a per-message 8-bit constant that guards against sender and
receiver having *different definitions* of the same message ID. It is
computed at code-generation time from the message name and the reordered
field list (type, name, and array length for arrays), **excluding**
extension fields, and stored in the generated `MAVLINK_MESSAGE_CRCS` table.

```python
def message_checksum(msg):
    crc = x25crc()
    crc.accumulate_str(msg.name + ' ')
    for f in msg.ordered_fields:          # reordered, base fields only
        crc.accumulate_str(f.type + ' ')  # type without array suffix, e.g. 'uint16_t'
        crc.accumulate_str(f.name + ' ')
        if f.array_length:
            crc.accumulate([f.array_length])
    return (crc.crc & 0xFF) ^ (crc.crc >> 8)
```

If a receiver's `CRC_EXTRA` for a message ID differs from the sender's, the
packet checksum will fail and the packet is dropped. Changing any base field
of a released message is therefore a breaking change; add extension fields
instead. Examples: `HEARTBEAT` = 50, `SYS_STATUS` = 124, `ATTITUDE` = 39,
`COMMAND_LONG` = 152, `COMMAND_ACK` = 143.

### 3.3 CRC-32 (FTP and PX4 parameter hash)

Source: <https://mavlink.io/en/guide/crc.html>

MAVLink FTP `CalcFileCRC32` and PX4's parameter hash use a CRC-32 that is
*like* the ISO 3309 / IEEE 802.3 polynomial `0x04C11DB7` **but** with initial
value `0` and **no** final XOR. It is unrelated to the CRC-16 above.

---

## Part 4 — Message Signing (Authentication)

Source: <https://mavlink.io/en/guide/message_signing.html>

Signing is a MAVLink 2 feature that lets a receiver verify a packet came from
a holder of a shared **secret key**. It provides **authenticity and
integrity**, **not confidentiality** — payloads remain plaintext.

### 4.1 Signature block (13 bytes, appended after checksum)

| Offset | Field | Size | Meaning |
|---|---|---|---|
| 0 | `link_id` | 1 | Identifies the link/channel the packet was sent on (per-sender counter of channels) |
| 1–6 | `timestamp` | 6 (48-bit LE) | Units of **10 µs** since **2015-01-01 00:00:00 UTC** (Unix offset 1 420 070 400 s) |
| 7–12 | `signature` | 6 | First 48 bits of SHA-256 |

Presence is signalled by `incompat_flags & 0x01` (`MAVLINK_IFLAG_SIGNED`).

### 4.2 Computing the signature

```
signature = SHA256( secret_key(32) || header(10) || payload || checksum(2) || link_id(1) || timestamp(6) )[0:6]
```

The header hashed is bytes 0–9 of the packet (STX through msgid, including
the flags with the signed bit set).

### 4.3 Secret key

- 32 bytes, held by both ends; typically derived by SHA-256 of a user
  passphrase or randomly generated.
- Must be stored persistently and **never** exposed over parameters, logs, or
  any public interface.
- May be provisioned with `SETUP_SIGNING` (msg 256: `target_system`,
  `target_component`, `secret_key[32]`, `initial_timestamp`) — **only over a
  trusted link** (USB, wired Ethernet), always unicast, never forwarded by
  routers, and stored to persistent storage by the recipient.

### 4.4 Timestamp rules

- Must be **monotonically increasing** per logical stream
  `(sysid, compid, link_id)`.
- On startup use `max(system_clock, last_persisted_timestamp)`; persist
  regularly (≈ every minute).
- Increment by at least 1 per packet if the clock has not advanced (allows
  bursts > 100 k pkt/s to run "ahead").
- If a **correctly signed** incoming packet has a newer timestamp, adopt it
  (clock sync). Never adopt timestamps from badly signed packets.

### 4.5 Acceptance rules for incoming packets

A signed packet is **rejected** if any of:

1. its timestamp is ≤ the last accepted timestamp from the same stream
   `(sysid, compid, link_id)`;
2. the computed signature does not match;
3. its timestamp is more than **1 minute (6 000 000 units)** older than the
   local timestamp.

**Unsigned** packets should be accepted only under a configurable policy,
e.g. by parameter, by transport (USB/Ethernet considered trusted), until the
first signed packet arrives on a link, or for specific message types
(`RADIO_STATUS` from an intermediate radio). Libraries expose a callback for
this (`accept_unsigned_callback` in C).

If an implementation ever accepts incorrectly signed traffic it must show a
highly conspicuous "insecure link" indication.

### 4.6 Logging

Do not log `SETUP_SIGNING` (or overwrite the key with 32 × `0xFF`). Strip
signature blocks and clear the signed flag before writing telemetry logs so
attackers cannot harvest signature material.

### 4.7 Limitations to be aware of

- No confidentiality, no key rotation protocol, symmetric key only.
- 48-bit truncated MAC; a compromised link exposes the shared key.
- One key per channel — every peer on a channel must share the same key.

These are the gaps JFOXLink's cryptographic envelope closes (Part 9).

---

## Part 5 — Versions and Negotiation

Sources: <https://mavlink.io/en/guide/mavlink_2.html>,
<https://mavlink.io/en/guide/mavlink_version.html>

### 5.1 MAVLink 2 vs MAVLink 1

| Feature | MAVLink 1 | MAVLink 2 |
|---|---|---|
| STX | `0xFE` | `0xFD` |
| Message IDs | 8-bit (0–255) | 24-bit (0–16 777 215) |
| Header overhead | 6 B + 2 B CRC = 8 B | 10 B + 2 B CRC = 12 B |
| Flags | none | `incompat_flags`, `compat_flags` |
| Extension fields | no | yes (backwards-compatible field additions) |
| Payload truncation | no | trailing zeros trimmed |
| Signing | no | 13-byte optional signature |
| Max packet | 263 B | 280 B |

MAVLink 2 is backwards compatible: v2 libraries parse v1 packets, and
messages with ID < 256 and no extensions can be sent in either framing.

### 5.2 Detecting peer version

1. **STX byte** — receiving a `0xFD` packet proves the peer speaks v2.
2. **`AUTOPILOT_VERSION.capabilities`** contains
   `MAV_PROTOCOL_CAPABILITY_MAVLINK2` (bit 15, value 32768).
3. **`HEARTBEAT.mavlink_version`** — the *minor* protocol version from the
   XML `<version>` (currently 3); not a v1/v2 indicator.
4. **`PROTOCOL_VERSION`** (msg 300, v2-only) — obtainable via
   `MAV_CMD_REQUEST_MESSAGE(300)` or `MAV_CMD_REQUEST_PROTOCOL_VERSION`.
   Fields: `version` (e.g. 200 = 2.0), `min_version`, `max_version`,
   `spec_version_hash[8]`, `library_version_hash[8]`.

### 5.3 Handshake for non-transparent links

If a link may re-serialize packets (e.g. a router that only speaks v1),
send `MAV_CMD_REQUEST_MESSAGE` for a **v2-only message** (ID ≥ 256, e.g.
`PROTOCOL_VERSION`). A v2-capable path returns the message framed as v2; a
v1-only path yields a NACK or nothing.

### 5.4 Recommended behaviour

- **Vehicles**: allow enabling v2 per channel; if signing is enabled, send
  signed v2 immediately; optionally start in v1 and switch when a v2 packet
  is received; advertise `MAV_PROTOCOL_CAPABILITY_MAVLINK2`.
- **GCS**: switch to v2 automatically on receipt of a v2 packet or the
  capability flag, or expose a setting.
- **All peers on one channel must use the same MAVLink version and the same
  signing key.**

---

## Part 6 — Routing and Addressing

Source: <https://mavlink.io/en/guide/routing.html>

### 6.1 Addressing

Every packet carries the sender's `sysid`/`compid` in the header. Messages
that are addressed also carry payload fields `target_system` and
`target_component`; a value of **0** (or the field's absence) means
**broadcast**.

| `target_system` | `target_component` | Meaning |
|---|---|---|
| absent / 0 | — | Network broadcast: everyone processes |
| = my sysid | absent / 0 | System broadcast: every component in my system processes |
| = my sysid | = my compid | Unicast to me |
| ≠ my sysid | — | Not for me (but maybe forward) |

`MAV_COMP_ID_ALL` = 0.

### 6.2 Processing rule

A component processes a message iff it is a network broadcast, a system
broadcast for its system, or addressed exactly to it. Broadcasts must
therefore be safe to act on by any component; anything with side effects
should be addressed.

### 6.3 Forwarding rule (routers / multi-channel components)

A component that bridges several channels forwards a message received on
one channel to another iff:

1. it is a network broadcast; **or**
2. `target_system` differs from its own and it has previously *seen* traffic
   from that system on the destination channel; **or**
3. `target_system` matches, `target_component` is set, and it has seen that
   system/component on the destination channel.

Routes are **learned** passively from the `sysid`/`compid` of incoming
packets (primarily heartbeats). Forwarded packets **must not be modified or
re-serialized**; the router simply copies bytes (this preserves signatures
and `seq`).

### 6.4 ID assignment conventions

Source: <https://mavlink.io/en/services/mavlink_id_assignment.html>

- Vehicles: `sysid` 1..; GCS conventionally 255 (QGC) / 250 range;
  companion computers/onboard components use the vehicle's `sysid`.
- Common `compid` values: `MAV_COMP_ID_AUTOPILOT1` = 1, user reserved 25–99,
  cameras 100–105, servos 140–153, gimbals 154 & 171–175, log 155, ADSB 156,
  OSD 157, peripheral 158, GPS 220/221, `MAV_COMP_ID_ONBOARD_COMPUTER` 191–194,
  `MAV_COMP_ID_MISSIONPLANNER` 190, `MAV_COMP_ID_PATHPLANNER` 195,
  `MAV_COMP_ID_UDP_BRIDGE` 240, `MAV_COMP_ID_UART_BRIDGE` 241,
  `MAV_COMP_ID_SYSTEM_CONTROL` 250.

---

## Part 7 — Message Definitions (XML Dialects)

Source: <https://mavlink.io/en/guide/xml_schema.html>,
<https://mavlink.io/en/messages/>

Messages, enums and commands are declared in XML and compiled to code with
`mavgen` (Python, `pymavlink/tools/mavgen.py`) or `mavgenerate` (GUI).

### 7.1 File structure

```xml
<?xml version="1.0"?>
<mavlink>
  <include>common.xml</include>       <!-- up to 5 nesting levels -->
  <version>3</version>                <!-- omit when including common.xml -->
  <dialect>0</dialect>
  <enums>
    <enum name="MY_ENUM" bitmask="true">
      <description>…</description>
      <entry value="1" name="MY_ENUM_A"><description>…</description></entry>
    </enum>
    <enum name="MAV_CMD">                <!-- commands are MAV_CMD entries -->
      <entry value="31010" name="MAV_CMD_MY_CMD" hasLocation="false" isDestination="false">
        <description>…</description>
        <param index="1" label="Foo" units="m" minValue="0">…</param>
        <!-- params 1..7 -->
      </entry>
    </enum>
  </enums>
  <messages>
    <message id="12000" name="MY_MESSAGE">
      <description>…</description>
      <field type="uint64_t" name="time_usec" units="us">…</field>
      <field type="uint8_t"  name="target_system">…</field>
      <field type="float[4]" name="q" invalid="[0]">…</field>
      <extensions/>
      <field type="uint8_t" name="new_flag" enum="MY_ENUM">…</field>
    </message>
  </messages>
</mavlink>
```

### 7.2 Field types

`char`, `uint8_t`, `int8_t`, `uint16_t`, `int16_t`, `uint32_t`, `int32_t`,
`uint64_t`, `int64_t`, `float`, `double`, and fixed arrays `type[N]`
(N ≤ 255; total payload ≤ 255 bytes). `uint8_t_mavlink_version` is a special
type auto-filled with the protocol minor version (used only in `HEARTBEAT`).

### 7.3 Field attributes

| Attribute | Purpose |
|---|---|
| `type`, `name` | required |
| `enum` | name of an enum whose values the field holds |
| `units` | e.g. `m`, `m/s`, `degE7`, `mm`, `rad`, `us`, `ms`, `cdeg` |
| `multiplier` | scaling to reach `units` (e.g. `1E-7`) |
| `display` | `bitmask` for bit-flag fields |
| `print_format` | printf-style hint |
| `instance` | `true` if field distinguishes multiple sensors/instances of same type |
| `invalid` | value meaning "not available" (e.g. `NaN`, `UINT16_MAX`, `[0]`, `[0:]`) |
| `default` | recommended default; `NaN` for floats means "use vehicle default" |
| `minValue`, `maxValue`, `increment` | value constraints for UIs |

### 7.4 Lifecycle tags (mutually exclusive)

- `<wip since="YYYY-MM"/>` — proposed, may change.
- `<superseded since="…" replaced_by="…"/>` — still valid, better option exists.
- `<deprecated since="…" replaced_by="…" remove_on_date="…"/>` — scheduled removal.

### 7.5 Message ID ranges

| Range | Use |
|---|---|
| 0–149, 230–255 | Reserved for `common.xml` (v1-compatible IDs) |
| 150–229 | Available for dialects needing MAVLink 1 IDs |
| 256–  | MAVLink 2 only; `common.xml` uses 256–~400 and 9000–12000 ranges; dialects should choose unused high ranges (e.g. 12900–12999 in ArduPilot) |
| MAV_CMD 0–30999 | Reserved for `common.xml`; 31000+ / vendor ranges for dialects |

Naming: `UPPER_SNAKE_CASE`; enum entries prefixed with enum name; commands
prefixed `MAV_CMD_`.

### 7.6 Standard dialects

| File | Purpose |
|---|---|
| `minimal.xml` | `HEARTBEAT`, `PROTOCOL_VERSION` and core enums — smallest interoperable set |
| `standard.xml` | Includes `minimal`; intended future "standard" set |
| `common.xml` | The de-facto shared message set (~300 messages, 170+ commands) |
| `development.xml` | Proposed additions under trial |
| `ardupilotmega.xml`, `px4.xml`, `ASLUAV.xml`, `uAvionix.xml`, `icarous.xml`, `storm32.xml`, `cubepilot.xml`, `AVSSUAS.xml`, `csAirLink.xml`, `all.xml` | Vendor / project dialects |

---

## Part 8 — Microservices

A microservice is a higher-level protocol layered on messages. Most follow a
request → response/acknowledge pattern with **sender-side timeout and retry**
(MAVLink itself is unreliable). Source index:
<https://mavlink.io/en/services/>

### 8.1 Heartbeat / Connection Protocol

Source: <https://mavlink.io/en/services/heartbeat.html>

`HEARTBEAT` (id 0, 9 bytes) is broadcast by **every component** at a nominal
**1 Hz**.

| Field | Type | Meaning |
|---|---|---|
| `type` | uint8 (`MAV_TYPE`) | Vehicle/component type (`MAV_TYPE_QUADROTOR`=2, `FIXED_WING`=1, `GCS`=6, `GIMBAL`=26, `CAMERA`=30, `ONBOARD_CONTROLLER`=18, `VTOL_*`=19–25 …) |
| `autopilot` | uint8 (`MAV_AUTOPILOT`) | Flight stack: `GENERIC`=0, `ARDUPILOTMEGA`=3, `PX4`=12, `INVALID`=8 (for non-FC components) |
| `base_mode` | uint8 bitmask (`MAV_MODE_FLAG`) | `CUSTOM_MODE_ENABLED`=1, `TEST`=2, `AUTO`=4, `GUIDED`=8, `STABILIZE`=16, `HIL`=32, `MANUAL_INPUT`=64, `SAFETY_ARMED`=128 |
| `custom_mode` | uint32 | Autopilot-specific flight mode |
| `system_status` | uint8 (`MAV_STATE`) | `UNINIT`=0, `BOOT`=1, `CALIBRATING`=2, `STANDBY`=3, `ACTIVE`=4, `CRITICAL`=5, `EMERGENCY`=6, `POWEROFF`=7, `FLIGHT_TERMINATION`=8 |
| `mavlink_version` | uint8 | Auto-filled protocol minor version (3) |

Rules:
- Component type is inferred from `HEARTBEAT.type`, **not** from `compid`.
- Flight controllers set a vehicle `MAV_TYPE` and a real `autopilot`; other
  components set their own type and `MAV_AUTOPILOT_INVALID`.
- Loss of **4–5 consecutive** expected heartbeats ⇒ treat as disconnected.
- Heartbeats drive route learning (Part 6) and GCS discovery of systems.

### 8.2 Command Protocol

Source: <https://mavlink.io/en/services/command.html>

Sends a `MAV_CMD_*` with up to 7 parameters and expects `COMMAND_ACK`.

**`COMMAND_LONG` (76)** — `target_system`, `target_component`, `command`
(uint16), `confirmation` (uint8), `param1..param7` (float).

**`COMMAND_INT` (75)** — `target_system`, `target_component`, `frame`
(`MAV_FRAME`), `command`, `current`, `autocontinue`, `param1..param4`
(float), `x`, `y` (int32; lat/lon × 1E7 or local × 1E4), `z` (float).
Preferred for **positional** commands: explicit frame, higher precision.

**`COMMAND_ACK` (77)** — `command`, `result` (`MAV_RESULT`), extensions:
`progress` (0–100, 255 = n/a), `result_param2` (int32, command-specific
detail), `target_system`, `target_component`.

`MAV_RESULT`: `ACCEPTED`=0, `TEMPORARILY_REJECTED`=1, `DENIED`=2,
`UNSUPPORTED`=3, `FAILED`=4, `IN_PROGRESS`=5, `CANCELLED`=6,
`COMMAND_LONG_ONLY`=7, `COMMAND_INT_ONLY`=8,
`COMMAND_UNSUPPORTED_MAV_FRAME`=9.

Sequence:

```
GCS ──COMMAND_LONG/INT (confirmation=0)──▶ Vehicle
GCS ◀──────────COMMAND_ACK(result)──────── Vehicle
   (no ACK within timeout → resend with confirmation++ ; give up after N tries)
Long-running:  ACK(IN_PROGRESS, progress=..) … ACK(ACCEPTED|FAILED|CANCELLED)
Cancel:        COMMAND_CANCEL(command) ──▶ ACK(CANCELLED)
```

- After `IN_PROGRESS`, the sender greatly extends its timeout.
- Only one instance of a long-running command may run; a duplicate is
  answered `TEMPORARILY_REJECTED`.
- If a command arrives in the wrong message type, reply
  `COMMAND_LONG_ONLY` / `COMMAND_INT_ONLY` so the sender can switch.

### 8.3 Mission Protocol

Source: <https://mavlink.io/en/services/mission.html>

Transfers three independent plan types, selected by `mission_type`
(`MAV_MISSION_TYPE_MISSION`=0, `FENCE`=1, `RALLY`=2, `ALL`=255).

**Mission item (`MISSION_ITEM_INT`, 73)** — `target_system`,
`target_component`, `seq` (uint16), `frame` (`MAV_FRAME`), `command`
(`MAV_CMD`), `current`, `autocontinue`, `param1..4` (float), `x`, `y`
(int32), `z` (float), `mission_type`. Global frames encode lat/lon as
degrees × 1E7; local frames as metres × 1E4. Use `*_INT` frames
(`MAV_FRAME_GLOBAL_INT`=5, `GLOBAL_RELATIVE_ALT_INT`=6,
`GLOBAL_TERRAIN_ALT_INT`=11). `MISSION_ITEM` (39) and `MISSION_REQUEST` (40)
are deprecated in favour of the INT variants; a vehicle receiving the legacy
message should still respond with the INT one.

**Upload (GCS → vehicle)**

```
GCS ──MISSION_COUNT(count, type)────────▶ Vehicle
GCS ◀─MISSION_REQUEST_INT(seq=0)────────  Vehicle
GCS ──MISSION_ITEM_INT(seq=0)───────────▶ Vehicle
        … repeat for seq=1..count-1 …
GCS ◀─MISSION_ACK(MAV_MISSION_ACCEPTED)─  Vehicle
```

**Download (vehicle → GCS)**

```
GCS ──MISSION_REQUEST_LIST(type)────────▶ Vehicle
GCS ◀─MISSION_COUNT(count)──────────────  Vehicle
GCS ──MISSION_REQUEST_INT(seq=0)────────▶ Vehicle
GCS ◀─MISSION_ITEM_INT(seq=0)───────────  Vehicle
        … repeat …
GCS ──MISSION_ACK(ACCEPTED)─────────────▶ Vehicle
```

**Other operations**: `MISSION_CLEAR_ALL` → `MISSION_ACK`;
`MAV_CMD_DO_SET_MISSION_CURRENT` (224) or `MISSION_SET_CURRENT` (41);
`MISSION_CURRENT` (42, streamed: `seq`, `total`, `mission_state`,
`mission_mode`, `mission_id`, `fence_id`, `rally_points_id`);
`MISSION_ITEM_REACHED` (46).

**Timing**: default timeout 1500 ms, per-item timeout 250 ms, up to 5
retries; on unrecoverable error the receiver sends `MISSION_ACK` with a
`MAV_MISSION_RESULT` error (`ERROR`=1, `UNSUPPORTED_FRAME`=2,
`UNSUPPORTED`=3, `NO_SPACE`=4, `INVALID`=5, `INVALID_PARAM1..7`=6–12,
`INVALID_SEQUENCE`=13, `DENIED`=14, `OPERATION_CANCELLED`=15) and the
previous plan is kept. Out-of-order items are re-requested, not fatal.

Plan-change detection: cache the opaque `mission_id`/`fence_id`/
`rally_points_id` from `MISSION_CURRENT` to skip unnecessary re-downloads.

### 8.4 Parameter Protocol

Source: <https://mavlink.io/en/services/parameter.html>

| Message | ID | Fields |
|---|---|---|
| `PARAM_REQUEST_READ` | 20 | `target_system`, `target_component`, `param_id[16]`, `param_index` (−1 to use id) |
| `PARAM_REQUEST_LIST` | 21 | `target_system`, `target_component` |
| `PARAM_VALUE` | 22 | `param_id[16]`, `param_value` (float), `param_type` (`MAV_PARAM_TYPE`), `param_count`, `param_index` |
| `PARAM_SET` | 23 | `target_system`, `target_component`, `param_id[16]`, `param_value`, `param_type` |

- `param_id`: ≤ 16 chars, NUL-terminated only if shorter than 16.
- `MAV_PARAM_TYPE`: `UINT8`=1, `INT8`=2, `UINT16`=3, `INT16`=4, `UINT32`=5,
  `INT32`=6, `UINT64`=7, `INT64`=8, `REAL32`=9, `REAL64`=10.
- **Encoding of non-float values in the float field**: *byte-wise* (bit
  pattern copied into the 4 bytes — lossless for ≤32-bit ints; used by
  ArduPilot, advertised by `MAV_PROTOCOL_CAPABILITY_PARAM_ENCODE_BYTEWISE`)
  vs *C-cast* (value converted to float — loses precision above 2^24; PX4,
  `…_PARAM_ENCODE_C_CAST`). GCS must check `AUTOPILOT_VERSION.capabilities`.
- Flows: `PARAM_REQUEST_LIST` → stream of all `PARAM_VALUE`; detect completion
  by `param_count`/index and a timeout after the last one, re-request missing
  indices individually. `PARAM_SET` is acknowledged by a `PARAM_VALUE`
  broadcast carrying the *actual* stored value; retry if no reply or value
  differs.
- Limitation: parameter set must be static during a session; broadcasts mean
  third-party components can miss updates. The **Extended Parameter
  Protocol** (`PARAM_EXT_*`, 320–324) supports string/custom types with
  explicit `PARAM_EXT_ACK`.

### 8.5 Telemetry and Message Rates

Vehicles stream telemetry (attitude, position, status…) as broadcasts. A GCS
tunes rates with:

- `MAV_CMD_SET_MESSAGE_INTERVAL` (511): `param1` = message id, `param2` =
  interval in µs (0 = default rate, −1 = disable); reply via `COMMAND_ACK`.
- `MAV_CMD_REQUEST_MESSAGE` (512): one-shot request for `param1` = message
  id (replaces the many `MAV_CMD_REQUEST_*` commands).
- `MAV_CMD_GET_MESSAGE_INTERVAL` (510) → `MESSAGE_INTERVAL` (244).
- Legacy: `REQUEST_DATA_STREAM` (66) with `MAV_DATA_STREAM` groups —
  deprecated.

Typical default rates over a 57600-baud telemetry radio: `HEARTBEAT` 1 Hz,
`SYS_STATUS` 1 Hz, `ATTITUDE` 10 Hz, `GLOBAL_POSITION_INT` 3–5 Hz,
`GPS_RAW_INT` 1–2 Hz, `RC_CHANNELS` 2 Hz, `VFR_HUD` 4 Hz.

### 8.6 Manual Control (Joystick)

Source: <https://mavlink.io/en/services/manual_control.html>

`MANUAL_CONTROL` (69): `target`, `x` (pitch), `y` (roll), `z` (thrust), `r`
(yaw) each int16 in **−1000…1000** (`INT16_MAX` = axis unused), `buttons`
(16-bit), extensions `buttons2`, `enabled_extensions`, `s`, `t`, `aux1..6`.
Extension axes `s`,`t` must be enabled by bits 0/1 of `enabled_extensions`.
Sent at ≥ 10 Hz typical; the vehicle fails-safe on loss. Alternative:
`RC_CHANNELS_OVERRIDE` (70) with 18 raw PWM channels — needs firmware setup,
less portable.

### 8.7 File Transfer Protocol (FTP)

Source: <https://mavlink.io/en/services/ftp.html>

All traffic uses `FILE_TRANSFER_PROTOCOL` (110): `target_network`,
`target_system`, `target_component`, `payload[251]`. Payload layout:

| Bytes | Field | Notes |
|---|---|---|
| 0–1 | `seq_number` | uint16, incremented per new command; retries reuse |
| 2 | `session` | Session handle for read/write |
| 3 | `opcode` | See below |
| 4 | `size` | Bytes of `data` used |
| 5 | `req_opcode` | In ACK/NAK: the opcode being answered |
| 6 | `burst_complete` | 1 on final packet of a burst read |
| 7 | `padding` | 32-bit alignment |
| 8–11 | `offset` | File offset / directory listing offset |
| 12–250 | `data[239]` | Path, file data, or error info |

Opcodes: `None`=0, `TerminateSession`=1, `ResetSessions`=2,
`ListDirectory`=3, `OpenFileRO`=4, `ReadFile`=5, `CreateFile`=6,
`WriteFile`=7, `RemoveFile`=8, `CreateDirectory`=9, `RemoveDirectory`=10,
`OpenFileWO`=11, `TruncateFile`=12, `Rename`=13, `CalcFileCRC32`=14,
`BurstReadFile`=15, `ACK`=128, `NAK`=129.

NAK error codes (in `data[0]`): `None`=0, `Fail`=1, `FailErrno`=2 (errno
in `data[1]`), `InvalidDataSize`=3, `InvalidSession`=4,
`NoSessionsAvailable`=5, `EOF`=6, `UnknownCommand`=7, `FileExists`=8,
`FileProtected`=9, `FileNotFound`=10.

Client-driven; ACK/NAK timeout ≈ 50 ms with up to 6 retries (implementation
dependent). Reads: `OpenFileRO` → repeated `ReadFile`/`BurstReadFile` →
`TerminateSession`. Writes: `CreateFile` → `WriteFile`… → `TerminateSession`.
Path URL scheme `mftp://[comp=<id>:][@<alias>/]<path>` (e.g. `@MAV_LOG`).

### 8.8 Camera Protocol v2

Source: <https://mavlink.io/en/services/camera.html>

1. Discover: camera sends `HEARTBEAT` (`MAV_TYPE_CAMERA`); GCS sends
   `MAV_CMD_REQUEST_MESSAGE(259)` → `CAMERA_INFORMATION` (flags
   `CAMERA_CAP_FLAGS_*`: capture image/video, has modes, zoom, focus,
   video stream, tracking…), `cam_definition_uri` → XML camera-definition
   file for vendor parameters.
2. Settings/mode: `CAMERA_SETTINGS` (260), `MAV_CMD_SET_CAMERA_MODE` (530).
3. Storage: `STORAGE_INFORMATION` (261), `MAV_CMD_STORAGE_FORMAT` (525).
4. Capture: `MAV_CMD_IMAGE_START_CAPTURE` (2000) / `STOP` (2001);
   `CAMERA_IMAGE_CAPTURED` (263) broadcast per image with geotag and
   `image_index`; `CAMERA_CAPTURE_STATUS` (262) streamed.
5. Video: `MAV_CMD_VIDEO_START_CAPTURE` (2500) / `STOP` (2501);
   `VIDEO_STREAM_INFORMATION` (269), `VIDEO_STREAM_STATUS` (270).
6. Zoom/focus/tracking: `MAV_CMD_SET_CAMERA_ZOOM` (531),
   `MAV_CMD_SET_CAMERA_FOCUS` (532), `MAV_CMD_CAMERA_TRACK_POINT` (2004),
   `…_TRACK_RECTANGLE` (2005), `CAMERA_TRACKING_IMAGE_STATUS` (275).

### 8.9 Gimbal Protocol v2

Source: <https://mavlink.io/en/services/gimbal_v2.html>

- **Gimbal Manager** (usually in the autopilot or companion) is the only
  party permitted to command a **Gimbal Device** (hardware); 1:1 pairing.
- Discovery: `MAV_PROTOCOL_CAPABILITY_COMPONENT_IMPLEMENTS_GIMBAL_MANAGER`
  → `MAV_CMD_REQUEST_MESSAGE(280)` → `GIMBAL_MANAGER_INFORMATION` (280);
  device: `GIMBAL_DEVICE_INFORMATION` (283).
- Control: `GIMBAL_MANAGER_SET_ATTITUDE` (282, quaternion + rates),
  `GIMBAL_MANAGER_SET_PITCHYAW` (287), `MAV_CMD_DO_GIMBAL_MANAGER_PITCHYAW`
  (1000), `MAV_CMD_DO_GIMBAL_MANAGER_CONFIGURE` (1001, primary/secondary
  control ownership). Manager translates to `GIMBAL_DEVICE_SET_ATTITUDE`
  (284).
- Status: `GIMBAL_MANAGER_STATUS` (281, ~5 Hz),
  `GIMBAL_DEVICE_ATTITUDE_STATUS` (285, ~10 Hz); flags `GIMBAL_MANAGER_FLAGS`
  / `GIMBAL_DEVICE_FLAGS` (retract, neutral, roll/pitch/yaw lock, yaw in
  vehicle/earth frame…).

### 8.10 Other Microservices

| Service | Key messages / commands | Purpose |
|---|---|---|
| Ping | `PING` (4) | Latency measurement, discovery |
| Time sync | `TIMESYNC` (111: `tc1`, `ts1`), `SYSTEM_TIME` (2) | Clock offset estimation |
| Arm authorization | `MAV_CMD_ARM_AUTHORIZATION_REQUEST` (3001) | External arming approval |
| Offboard control | `SET_POSITION_TARGET_LOCAL_NED` (84), `…_GLOBAL_INT` (86), `SET_ATTITUDE_TARGET` (82) with `type_mask` | Companion-computer control |
| Landing target | `LANDING_TARGET` (149) | Precision landing |
| Battery | `BATTERY_STATUS` (147), `BATTERY_INFO` | Multi-battery reporting |
| Terrain | `TERRAIN_REQUEST` (133), `TERRAIN_DATA` (134), `TERRAIN_CHECK`, `TERRAIN_REPORT` | GCS-supplied terrain tiles |
| Tunnel | `TUNNEL` (385) | Opaque vendor payloads (`payload_type`, up to 128 B) |
| Image transmission | `DATA_TRANSMISSION_HANDSHAKE` (130), `ENCAPSULATED_DATA` (131) | Chunked image transfer |
| High latency | `HIGH_LATENCY2` (235) | Compressed status for satcom links |
| Component metadata | `COMPONENT_METADATA` (397) via FTP → JSON | Machine-readable capabilities |
| Open Drone ID | `OPEN_DRONE_ID_*` (12900–12920) | Remote ID broadcast |
| Traffic management | `ADSB_VEHICLE` (246), `UTM_GLOBAL_POSITION` (340) | Airspace awareness |
| Standard modes | `MAV_CMD_DO_SET_STANDARD_MODE` (262), `AVAILABLE_MODES` (435), `CURRENT_MODE` (436) | Stack-agnostic flight modes |
| Events | `EVENT` (410), `CURRENT_EVENT_SEQUENCE` (411) | Structured event log (WIP) |
| Payload | `MAV_CMD_DO_SET_ACTUATOR`, `MAV_CMD_DO_GRIPPER` etc. | Generic payload actuation |
| Illuminator | `ILLUMINATOR_STATUS` (440), `MAV_CMD_ILLUMINATOR_ON_OFF` | Lighting |

---

## Part 9 — MAVLink and JFOXLink

JFOXLink (this repository) is derived from MAVLink 2 and is designed to
carry MAVLink 2 payloads transparently while adding what MAVLink lacks for
contested environments. Relevant repo code: `crates/jfl-core/src/frame.rs`,
`crates/jfl-core/src/mavlink_compat.rs`, `crates/jfl-core/src/native.rs`.

### 9.1 What is inherited

| MAVLink 2 concept | JFOXLink |
|---|---|
| STX `0xFD` | Same (`JFL_STX = 0xFD`) |
| `len`, `incompat_flags`, `compat_flags`, `seq`, `sysid`, `compid`, `msgid[3]` (bytes 0–9) | Identical layout at bytes 0–9 — a plain MAVLink 2 parser can sync on and identify a JFOXLink frame |
| `MAVLINK_IFLAG_SIGNED` = 0x01 | Retained; JFOXLink adds `JFOX_CRYPTO_ACTIVE` = 0x02 as a second **incompat** flag so legacy MAVLink parsers correctly **discard** encrypted frames instead of mis-decoding them |
| Routing by `sysid`/`compid`/`target_*` | Same addressing model inside the payload |
| Microservices (heartbeat, command, mission, parameter, …) | Carried unchanged inside the encrypted payload when a MAVLink 2 message is wrapped |

### 9.2 What is added

After the 10-byte MAVLink header, JFOXLink inserts `jfl_version` (1 B),
`nonce` (12 B), `channel_flags` (1 B) — a 24-byte header total — followed by
the **encrypted payload**, a 16-byte **AES-256-GCM tag** and a 32-byte
**HMAC**. Compare with MAVLink signing (Part 4):

| Property | MAVLink 2 signing | JFOXLink envelope |
|---|---|---|
| Confidentiality | none | AES-256-GCM |
| Integrity/authenticity | 48-bit truncated SHA-256 | 128-bit GCM tag + 256-bit HMAC |
| Replay protection | 48-bit timestamp, 1-minute window | 96-bit nonce + replay window (`JflError::ReplayDetected`) |
| Key agreement | out-of-band `SETUP_SIGNING` | ECDH (P-384) + HKDF |
| Link redundancy / anti-jam | none | dual-channel `channel_flags`, FHSS/DSSS at HAL |

### 9.3 Interop rules of thumb

- A JFOXLink node speaking to a legacy MAVLink 2 peer must strip the
  envelope and re-emit standard frames (the `mavlink_compat` shim); routers
  in between must not re-serialize (Part 6.3), so encryption should be
  end-to-end between endpoints that both hold keys.
- Keep MAVLink's per-channel invariant (Part 5.4): every peer on a channel
  uses the same framing (JFOXLink or plain) and the same key material.
- Reuse MAVLink microservice timing (Part 8) unchanged; encryption adds
  bytes, not round trips.

---

## Appendix A — Key Messages Reference

IDs from `common.xml`. Fields listed in XML order (wire order is reordered per
Part 2.3). `[ext]` = MAVLink 2 extension.

| ID | Message | Key fields | Notes |
|---|---|---|---|
| 0 | `HEARTBEAT` | type, autopilot, base_mode, custom_mode, system_status, mavlink_version | 1 Hz, CRC_EXTRA 50 |
| 1 | `SYS_STATUS` | onboard_control_sensors_present/enabled/health (bitmasks), load, voltage_battery, current_battery, battery_remaining, drop_rate_comm, errors_comm | |
| 2 | `SYSTEM_TIME` | time_unix_usec, time_boot_ms | |
| 4 | `PING` | time_usec, seq, target_system, target_component | |
| 11 | `SET_MODE` | target_system, base_mode, custom_mode | deprecated → `MAV_CMD_DO_SET_MODE` |
| 20–23 | `PARAM_REQUEST_READ`, `PARAM_REQUEST_LIST`, `PARAM_VALUE`, `PARAM_SET` | see 8.4 | |
| 24 | `GPS_RAW_INT` | time_usec, fix_type, lat/lon (degE7), alt (mm), eph, epv, vel (cm/s), cog (cdeg), satellites_visible, [ext] alt_ellipsoid, h_acc, v_acc, vel_acc, hdg_acc, yaw | |
| 27 | `RAW_IMU` | xacc…zgyro…zmag, [ext] id, temperature | |
| 29 | `SCALED_PRESSURE` | press_abs, press_diff, temperature | |
| 30 | `ATTITUDE` | time_boot_ms, roll, pitch, yaw (rad), rollspeed, pitchspeed, yawspeed (rad/s) | |
| 31 | `ATTITUDE_QUATERNION` | q1–q4, rates, [ext] repr_offset_q | |
| 32 | `LOCAL_POSITION_NED` | x, y, z, vx, vy, vz | m, m/s |
| 33 | `GLOBAL_POSITION_INT` | time_boot_ms, lat/lon (degE7), alt (mm MSL), relative_alt (mm), vx/vy/vz (cm/s), hdg (cdeg) | fused estimate |
| 35 | `RC_CHANNELS_RAW` | chan1–8_raw, rssi | |
| 36 | `SERVO_OUTPUT_RAW` | servo1–8_raw, [ext] servo9–16_raw | |
| 39/40 | `MISSION_ITEM` / `MISSION_REQUEST` | deprecated | use INT variants |
| 41 | `MISSION_SET_CURRENT` | seq | |
| 42 | `MISSION_CURRENT` | seq, [ext] total, mission_state, mission_mode, mission_id, fence_id, rally_points_id | |
| 43 | `MISSION_REQUEST_LIST` | mission_type | |
| 44 | `MISSION_COUNT` | count, mission_type, [ext] opaque_id | |
| 45 | `MISSION_CLEAR_ALL` | mission_type | |
| 46 | `MISSION_ITEM_REACHED` | seq | |
| 47 | `MISSION_ACK` | type (`MAV_MISSION_RESULT`), mission_type, [ext] opaque_id | |
| 48 | `SET_GPS_GLOBAL_ORIGIN` | lat, lon, alt | |
| 51 | `MISSION_REQUEST_INT` | seq, mission_type | |
| 65 | `RC_CHANNELS` | chancount, chan1–18_raw, rssi | |
| 66 | `REQUEST_DATA_STREAM` | deprecated | use SET_MESSAGE_INTERVAL |
| 69 | `MANUAL_CONTROL` | x, y, z, r, buttons, [ext] buttons2, enabled_extensions, s, t, aux1–6 | |
| 70 | `RC_CHANNELS_OVERRIDE` | chan1–8_raw, [ext] chan9–18_raw | |
| 73 | `MISSION_ITEM_INT` | see 8.3 | |
| 74 | `VFR_HUD` | airspeed, groundspeed, heading, throttle, alt, climb | |
| 75 | `COMMAND_INT` | see 8.2 | |
| 76 | `COMMAND_LONG` | see 8.2 | |
| 77 | `COMMAND_ACK` | command, result, [ext] progress, result_param2, target_system, target_component | |
| 80 | `COMMAND_CANCEL` | command | |
| 82 | `SET_ATTITUDE_TARGET` | type_mask, q[4], body rates, thrust, [ext] thrust_body[3] | offboard |
| 84 | `SET_POSITION_TARGET_LOCAL_NED` | coordinate_frame, type_mask, x/y/z, vx/vy/vz, afx/afy/afz, yaw, yaw_rate | offboard |
| 86 | `SET_POSITION_TARGET_GLOBAL_INT` | lat_int, lon_int, alt, … | offboard |
| 87 | `POSITION_TARGET_GLOBAL_INT` | current setpoint | |
| 105 | `HIGHRES_IMU` | full IMU set + fields_updated | |
| 109 | `RADIO_STATUS` | rssi, remrssi, txbuf, noise, remnoise, rxerrors, fixed | sent by radios (unsigned) |
| 110 | `FILE_TRANSFER_PROTOCOL` | see 8.7 | |
| 111 | `TIMESYNC` | tc1, ts1, [ext] target_system, target_component | |
| 116 | `SCALED_IMU2` | | |
| 124 | `GPS2_RAW` | | |
| 125 | `POWER_STATUS` | Vcc, Vservo, flags | |
| 141 | `ALTITUDE` | monotonic, amsl, local, relative, terrain, bottom_clearance | |
| 147 | `BATTERY_STATUS` | id, battery_function, type, temperature, voltages[10], current_battery, current_consumed, energy_consumed, battery_remaining, [ext] time_remaining, charge_state, voltages_ext[4], mode, fault_bitmask | |
| 148 | `AUTOPILOT_VERSION` | capabilities (`MAV_PROTOCOL_CAPABILITY`), flight/middleware/os_sw_version, board_version, custom versions, vendor_id, product_id, uid, [ext] uid2[18] | |
| 149 | `LANDING_TARGET` | | |
| 230 | `ESTIMATOR_STATUS` | | |
| 231 | `WIND_COV` | | |
| 233 | `GPS_RTCM_DATA` | flags, len, data[180] | RTK corrections |
| 234 | `HIGH_LATENCY` / 235 `HIGH_LATENCY2` | | satcom |
| 241 | `VIBRATION` | | |
| 242 | `HOME_POSITION` | lat, lon, alt, x, y, z, q, approach vector, [ext] time_usec | |
| 244 | `MESSAGE_INTERVAL` | message_id, interval_us | |
| 245 | `EXTENDED_SYS_STATE` | vtol_state, landed_state | |
| 246 | `ADSB_VEHICLE` | | |
| 247 | `COLLISION` | | |
| 248 | `V2_EXTENSION` | | |
| 249 | `MEMORY_VECT` | | |
| 250 | `DEBUG_VECT` | | |
| 251–254 | `NAMED_VALUE_FLOAT`, `NAMED_VALUE_INT`, `STATUSTEXT`, `DEBUG` | `STATUSTEXT`: severity (`MAV_SEVERITY`), text[50], [ext] id, chunk_seq | |
| 256 | `SETUP_SIGNING` | target_system, target_component, secret_key[32], initial_timestamp | trusted links only |
| 257 | `BUTTON_CHANGE` | | |
| 258 | `PLAY_TUNE` | | |
| 259–275 | `CAMERA_INFORMATION`, `CAMERA_SETTINGS`, `STORAGE_INFORMATION`, `CAMERA_CAPTURE_STATUS`, `CAMERA_IMAGE_CAPTURED`, `FLIGHT_INFORMATION`, `MOUNT_ORIENTATION`, `LOGGING_DATA`, `LOGGING_DATA_ACKED`, `LOGGING_ACK`, `VIDEO_STREAM_INFORMATION`, `VIDEO_STREAM_STATUS`, `CAMERA_FOV_STATUS`, `CAMERA_TRACKING_IMAGE_STATUS`, `CAMERA_TRACKING_GEO_STATUS` | | |
| 280–287 | `GIMBAL_MANAGER_*`, `GIMBAL_DEVICE_*` | see 8.9 | |
| 300 | `PROTOCOL_VERSION` | version, min_version, max_version, spec_version_hash[8], library_version_hash[8] | |
| 310 | `UAVCAN_NODE_STATUS` | | |
| 320–324 | `PARAM_EXT_REQUEST_READ`, `PARAM_EXT_REQUEST_LIST`, `PARAM_EXT_VALUE`, `PARAM_EXT_SET`, `PARAM_EXT_ACK` | extended parameters | |
| 330 | `OBSTACLE_DISTANCE` | | |
| 331 | `ODOMETRY` | | |
| 332 | `TRAJECTORY_REPRESENTATION_WAYPOINTS` | | |
| 335 | `ISBD_LINK_STATUS` | | |
| 339 | `RAW_RPM` | | |
| 340 | `UTM_GLOBAL_POSITION` | | |
| 350 | `DEBUG_FLOAT_ARRAY` | | |
| 360 | `ORBIT_EXECUTION_STATUS` | | |
| 370 | `SMART_BATTERY_INFO` / `BATTERY_INFO` | | |
| 373 | `GENERATOR_STATUS` | | |
| 375 | `ACTUATOR_OUTPUT_STATUS` | | |
| 380 | `TIME_ESTIMATE_TO_TARGET` | | |
| 385 | `TUNNEL` | target_system, target_component, payload_type, payload_length, payload[128] | |
| 386–388 | `CAN_FRAME`, `CANFD_FRAME`, `CAN_FILTER_MODIFY` | | |
| 390 | `ONBOARD_COMPUTER_STATUS` | | |
| 395 | `COMPONENT_INFORMATION` (deprecated) / 397 `COMPONENT_METADATA` | | |
| 400 | `PLAY_TUNE_V2` | | |
| 401 | `SUPPORTED_TUNES` | | |
| 410–412 | `EVENT`, `CURRENT_EVENT_SEQUENCE`, `REQUEST_EVENT` | | |
| 435/436 | `AVAILABLE_MODES`, `CURRENT_MODE` | | |
| 9000 | `WHEEL_DISTANCE` | | |
| 9005 | `WINCH_STATUS` | | |
| 12900–12920 | `OPEN_DRONE_ID_*` | | |

## Appendix B — Key Enums

**`MAV_TYPE`** (vehicle/component): GENERIC 0, FIXED_WING 1, QUADROTOR 2,
COAXIAL 3, HELICOPTER 4, ANTENNA_TRACKER 5, GCS 6, AIRSHIP 7,
FREE_BALLOON 8, ROCKET 9, GROUND_ROVER 10, SURFACE_BOAT 11, SUBMARINE 12,
HEXAROTOR 13, OCTOROTOR 14, TRICOPTER 15, FLAPPING_WING 16, KITE 17,
ONBOARD_CONTROLLER 18, VTOL_TAILSITTER_DUOROTOR 19,
VTOL_TAILSITTER_QUADROTOR 20, VTOL_TILTROTOR 21, VTOL_FIXEDROTOR 22,
VTOL_TAILSITTER 23, VTOL_TILTWING 24, VTOL_RESERVED5 25, GIMBAL 26, ADSB 27,
PARAFOIL 28, DODECAROTOR 29, CAMERA 30, CHARGING_STATION 31, FLARM 32,
SERVO 33, ODID 34, DECAROTOR 35, BATTERY 36, PARACHUTE 37, LOG 38, OSD 39,
IMU 40, GPS 41, WINCH 42, GENERIC_MULTIROTOR 43, ILLUMINATOR 44.

**`MAV_AUTOPILOT`**: GENERIC 0, RESERVED 1, SLUGS 2, ARDUPILOTMEGA 3,
OPENPILOT 4, GENERIC_WAYPOINTS_ONLY 5, GENERIC_WAYPOINTS_AND_SIMPLE_NAVIGATION_ONLY 6,
GENERIC_MISSION_FULL 7, INVALID 8, PPZ 9, UDB 10, FP 11, PX4 12, SMACCMPILOT 13,
AUTOQUAD 14, ARMAZILA 15, AEROB 16, ASLUAV 17, SMARTAP 18, AIRRAILS 19, REFLEX 20.

**`MAV_STATE`**: UNINIT 0, BOOT 1, CALIBRATING 2, STANDBY 3, ACTIVE 4,
CRITICAL 5, EMERGENCY 6, POWEROFF 7, FLIGHT_TERMINATION 8.

**`MAV_MODE_FLAG`** (bitmask): CUSTOM_MODE_ENABLED 1, TEST_ENABLED 2,
AUTO_ENABLED 4, GUIDED_ENABLED 8, STABILIZE_ENABLED 16, HIL_ENABLED 32,
MANUAL_INPUT_ENABLED 64, SAFETY_ARMED 128.

**`MAV_RESULT`**: ACCEPTED 0, TEMPORARILY_REJECTED 1, DENIED 2,
UNSUPPORTED 3, FAILED 4, IN_PROGRESS 5, CANCELLED 6, COMMAND_LONG_ONLY 7,
COMMAND_INT_ONLY 8, COMMAND_UNSUPPORTED_MAV_FRAME 9.

**`MAV_FRAME`**: GLOBAL 0, LOCAL_NED 1, MISSION 2, GLOBAL_RELATIVE_ALT 3,
LOCAL_ENU 4, GLOBAL_INT 5, GLOBAL_RELATIVE_ALT_INT 6, LOCAL_OFFSET_NED 7,
BODY_NED 8 (deprecated), BODY_OFFSET_NED 9 (deprecated),
GLOBAL_TERRAIN_ALT 10, GLOBAL_TERRAIN_ALT_INT 11, BODY_FRD 12,
LOCAL_FRD 20, LOCAL_FLU 21.

**`MAV_SEVERITY`** (syslog): EMERGENCY 0, ALERT 1, CRITICAL 2, ERROR 3,
WARNING 4, NOTICE 5, INFO 6, DEBUG 7.

**`MAV_PROTOCOL_CAPABILITY`** (bitmask in `AUTOPILOT_VERSION.capabilities`):
MISSION_FLOAT 1, PARAM_FLOAT 2 (deprecated), MISSION_INT 4, COMMAND_INT 8,
PARAM_ENCODE_BYTEWISE 16, FTP 32, SET_ATTITUDE_TARGET 64,
SET_POSITION_TARGET_LOCAL_NED 128, SET_POSITION_TARGET_GLOBAL_INT 256,
TERRAIN 512, RESERVED3 1024, FLIGHT_TERMINATION 2048,
COMPASS_CALIBRATION 4096, MAVLINK2 32768, MISSION_FENCE 65536,
MISSION_RALLY 131072, RESERVED2 262144, PARAM_ENCODE_C_CAST 524288,
COMPONENT_IMPLEMENTS_GIMBAL_MANAGER 1048576.

**`MAV_MISSION_TYPE`**: MISSION 0, FENCE 1, RALLY 2, ALL 255.

**`MAV_PARAM_TYPE`**: UINT8 1, INT8 2, UINT16 3, INT16 4, UINT32 5, INT32 6,
UINT64 7, INT64 8, REAL32 9, REAL64 10.

**`MAV_COMPONENT`** (selected): ALL 0, AUTOPILOT1 1, USER1–75 25–99,
CAMERA–CAMERA6 100–105, SERVO1–14 140–153, GIMBAL 154, LOG 155, ADSB 156,
OSD 157, PERIPHERAL 158, QX1_GIMBAL 159, FLARM 160, PARACHUTE 161, WINCH 169,
GIMBAL2–6 171–175, BATTERY/BATTERY2 180–181, MAVCAN 189, MISSIONPLANNER 190,
ONBOARD_COMPUTER–4 191–194, PATHPLANNER 195, OBSTACLE_AVOIDANCE 196,
VISUAL_INERTIAL_ODOMETRY 197, PAIRING_MANAGER 198, IMU/IMU_2/IMU_3 200–202,
GPS/GPS2 220–221, ODID_TXRX_1–3 236–238, UDP_BRIDGE 240, UART_BRIDGE 241,
TUNNEL_NODE 242, ILLUMINATOR 243, SYSTEM_CONTROL 250.

## Appendix C — Frequently Used MAV_CMD Commands

Param columns follow the XML `<param index="n">` labels; unused params are
omitted. `x/y/z` refer to `COMMAND_INT`/mission-item positional fields
(param5–7 in `COMMAND_LONG`).

| ID | Command | Params |
|---|---|---|
| 16 | `MAV_CMD_NAV_WAYPOINT` | 1 hold s, 2 accept radius m, 3 pass radius m, 4 yaw deg, x lat, y lon, z alt |
| 17 | `MAV_CMD_NAV_LOITER_UNLIM` | 3 radius m, 4 yaw, x/y/z |
| 18 | `MAV_CMD_NAV_LOITER_TURNS` | 1 turns, 2 heading required, 3 radius, 4 xtrack, x/y/z |
| 19 | `MAV_CMD_NAV_LOITER_TIME` | 1 time s, 2 heading required, 3 radius, 4 xtrack, x/y/z |
| 20 | `MAV_CMD_NAV_RETURN_TO_LAUNCH` | — |
| 21 | `MAV_CMD_NAV_LAND` | 1 abort alt, 2 land mode, 4 yaw, x/y/z |
| 22 | `MAV_CMD_NAV_TAKEOFF` | 1 pitch, 4 yaw, x/y/z |
| 30 | `MAV_CMD_NAV_CONTINUE_AND_CHANGE_ALT` | 1 action, z alt |
| 31 | `MAV_CMD_NAV_LOITER_TO_ALT` | 1 heading required, 2 radius, 4 xtrack, x/y/z |
| 34 | `MAV_CMD_DO_ORBIT` | 1 radius, 2 velocity, 3 yaw behavior, 4 orbits, x/y/z |
| 84 | `MAV_CMD_NAV_VTOL_TAKEOFF` | 2 transition heading, 4 yaw, x/y/z |
| 85 | `MAV_CMD_NAV_VTOL_LAND` | 1 land options, 3 approach alt, 4 yaw, x/y/z |
| 92 | `MAV_CMD_NAV_GUIDED_ENABLE` | 1 enable (>0.5) |
| 93 | `MAV_CMD_NAV_DELAY` | 1 delay s, 2 hour, 3 minute, 4 second |
| 112 | `MAV_CMD_CONDITION_DELAY` | 1 delay s |
| 113 | `MAV_CMD_CONDITION_CHANGE_ALT` | 1 rate, z alt |
| 114 | `MAV_CMD_CONDITION_DISTANCE` | 1 distance m |
| 115 | `MAV_CMD_CONDITION_YAW` | 1 angle deg, 2 rate deg/s, 3 direction, 4 relative |
| 176 | `MAV_CMD_DO_SET_MODE` | 1 base_mode, 2 custom_mode, 3 custom_submode |
| 177 | `MAV_CMD_DO_JUMP` | 1 seq, 2 repeat count |
| 178 | `MAV_CMD_DO_CHANGE_SPEED` | 1 speed type, 2 speed m/s, 3 throttle % |
| 179 | `MAV_CMD_DO_SET_HOME` | 1 use current, 4 yaw, x/y/z |
| 183 | `MAV_CMD_DO_SET_SERVO` | 1 instance, 2 PWM |
| 184 | `MAV_CMD_DO_REPEAT_SERVO` | 1 instance, 2 PWM, 3 count, 4 period |
| 187 | `MAV_CMD_DO_SET_ACTUATOR` | 1–6 actuator values, 7 index |
| 189 | `MAV_CMD_DO_LAND_START` | x/y |
| 192 | `MAV_CMD_DO_REPOSITION` | 1 speed, 2 bitmask, 3 radius, 4 yaw, x/y/z |
| 193 | `MAV_CMD_DO_PAUSE_CONTINUE` | 1 continue (0 pause / 1 continue) |
| 195 | `MAV_CMD_DO_SET_ROI_LOCATION` | 1 gimbal device id, x/y/z |
| 197 | `MAV_CMD_DO_SET_ROI_NONE` | 1 gimbal device id |
| 201 | `MAV_CMD_DO_SET_ROI` | (deprecated) |
| 203 | `MAV_CMD_DO_DIGICAM_CONTROL` | legacy camera trigger |
| 206 | `MAV_CMD_DO_SET_CAM_TRIGG_DIST` | 1 distance m, 2 shutter, 3 trigger once |
| 207 | `MAV_CMD_DO_FENCE_ENABLE` | 1 enable |
| 208 | `MAV_CMD_DO_PARACHUTE` | 1 action (0 disable,1 enable,2 release) |
| 209 | `MAV_CMD_DO_MOTOR_TEST` | 1 instance, 2 throttle type, 3 throttle, 4 timeout, 5 count, 6 order |
| 210 | `MAV_CMD_DO_INVERTED_FLIGHT` | 1 inverted |
| 224 | `MAV_CMD_DO_SET_MISSION_CURRENT` | 1 number, 2 reset mission |
| 241 | `MAV_CMD_PREFLIGHT_CALIBRATION` | 1 gyro, 2 mag, 3 pressure, 4 RC, 5 accel, 6 compmot/airspeed, 7 esc |
| 242 | `MAV_CMD_PREFLIGHT_SET_SENSOR_OFFSETS` | |
| 245 | `MAV_CMD_PREFLIGHT_STORAGE` | 1 param storage (0 read,1 write,2 reset), 2 mission storage |
| 246 | `MAV_CMD_PREFLIGHT_REBOOT_SHUTDOWN` | 1 autopilot (1 reboot, 2 shutdown), 2 companion |
| 252 | `MAV_CMD_OVERRIDE_GOTO` | |
| 262 | `MAV_CMD_DO_SET_STANDARD_MODE` | 1 `MAV_STANDARD_MODE` |
| 300 | `MAV_CMD_MISSION_START` | 1 first item, 2 last item |
| 400 | `MAV_CMD_COMPONENT_ARM_DISARM` | 1 arm (0/1), 2 force (21196) |
| 401 | `MAV_CMD_RUN_PREARM_CHECKS` | — |
| 405 | `MAV_CMD_ILLUMINATOR_ON_OFF` | 1 enable |
| 410 | `MAV_CMD_GET_HOME_POSITION` | (deprecated → REQUEST_MESSAGE 242) |
| 500 | `MAV_CMD_START_RX_PAIR` | |
| 510 | `MAV_CMD_GET_MESSAGE_INTERVAL` | 1 message id |
| 511 | `MAV_CMD_SET_MESSAGE_INTERVAL` | 1 message id, 2 interval µs, 7 response target |
| 512 | `MAV_CMD_REQUEST_MESSAGE` | 1 message id, 2–6 index/params, 7 response target |
| 519 | `MAV_CMD_REQUEST_PROTOCOL_VERSION` | (deprecated → 512/300) |
| 520 | `MAV_CMD_REQUEST_AUTOPILOT_CAPABILITIES` | (deprecated → 512/148) |
| 521–532 | `REQUEST_CAMERA_INFORMATION`…`SET_CAMERA_FOCUS` | camera (see 8.8) |
| 600 | `MAV_CMD_JUMP_TAG` / 601 `DO_JUMP_TAG` | 1 tag |
| 1000 | `MAV_CMD_DO_GIMBAL_MANAGER_PITCHYAW` | 1 pitch, 2 yaw, 3 pitch rate, 4 yaw rate, 5 flags, 7 gimbal id |
| 1001 | `MAV_CMD_DO_GIMBAL_MANAGER_CONFIGURE` | 1–4 primary/secondary sysid/compid, 7 gimbal id |
| 2000 | `MAV_CMD_IMAGE_START_CAPTURE` | 2 interval s, 3 count, 4 seq |
| 2001 | `MAV_CMD_IMAGE_STOP_CAPTURE` | |
| 2003 | `MAV_CMD_DO_TRIGGER_CONTROL` | 1 enable, 2 reset, 3 pause |
| 2004/2005 | `MAV_CMD_CAMERA_TRACK_POINT` / `_RECTANGLE` | normalized coords |
| 2500/2501 | `MAV_CMD_VIDEO_START_CAPTURE` / `_STOP_CAPTURE` | 1 stream id |
| 2502/2503 | `MAV_CMD_VIDEO_START_STREAMING` / `_STOP_STREAMING` | 1 stream id |
| 2510/2511 | `MAV_CMD_LOGGING_START` / `_STOP` | |
| 2800/2801 | `MAV_CMD_PANORAMA_CREATE` / … | |
| 3000 | `MAV_CMD_DO_VTOL_TRANSITION` | 1 `MAV_VTOL_STATE` (3 MC, 4 FW), 2 immediate |
| 3001 | `MAV_CMD_ARM_AUTHORIZATION_REQUEST` | 1 sysid |
| 4000/4001 | `MAV_CMD_SET_GUIDED_SUBMODE_STANDARD` / `_CIRCLE` | |
| 4501 | `MAV_CMD_CONDITION_GATE` | |
| 5000 | `MAV_CMD_NAV_FENCE_RETURN_POINT` | x/y/z |
| 5001 | `MAV_CMD_NAV_FENCE_POLYGON_VERTEX_INCLUSION` | 1 vertex count, 2 group, x/y |
| 5002 | `MAV_CMD_NAV_FENCE_POLYGON_VERTEX_EXCLUSION` | 1 vertex count, x/y |
| 5003 | `MAV_CMD_NAV_FENCE_CIRCLE_INCLUSION` | 1 radius, 2 group, x/y |
| 5004 | `MAV_CMD_NAV_FENCE_CIRCLE_EXCLUSION` | 1 radius, x/y |
| 5100 | `MAV_CMD_NAV_RALLY_POINT` | x/y/z |
| 5200 | `MAV_CMD_UAVCAN_GET_NODE_INFO` | |
| 30001/30002 | `MAV_CMD_PAYLOAD_PREPARE_DEPLOY` / `_CONTROL_DEPLOY` | |
| 31000–31014 | `MAV_CMD_WAYPOINT_USER_1..5`, `SPATIAL_USER_1..5`, `USER_1..5` | user-defined |

## Appendix D — Worked Example: Encoding a HEARTBEAT

`HEARTBEAT` XML order: `type` u8, `autopilot` u8, `base_mode` u8,
`custom_mode` u32, `system_status` u8, `mavlink_version` u8.
Wire order after reordering (4-byte first): `custom_mode`, `type`,
`autopilot`, `base_mode`, `system_status`, `mavlink_version` — 9 bytes.

Example values: quadrotor (2), PX4 (12), base_mode = ARMED|CUSTOM (0x81),
custom_mode = 0x00030000, STANDBY (3), version 3, sent by sysid 1 / compid 1,
seq 0, MAVLink 2, unsigned.

```
FD 09 00 00 00 01 01 00 00 00   ← STX, len=9, iflags, cflags, seq, sysid, compid, msgid=0
00 00 03 00 02 0C 81 03 03      ← custom_mode(LE) type autopilot base_mode status version
xx xx                           ← CRC-16/MCRF4XX over bytes 1..18 then CRC_EXTRA=50, LE
```

Notes: `custom_mode` low bytes are zero but sit at the *start* of the payload,
so no truncation applies here; had `mavlink_version` and `system_status`
been zero, `len` would shrink to 7 (first byte never trimmed). Signed frames
would set `iflags = 01` and append 13 bytes after the CRC.

## Appendix E — Implementation Checklist

**Parser**
- [ ] Byte-wise state machine keyed on STX `0xFD`/`0xFE`; resync on failure.
- [ ] Reject unknown `incompat_flags`; ignore unknown `compat_flags`.
- [ ] Look up `CRC_EXTRA` by `msgid`; drop packets with unknown ids or CRC
      mismatch; zero-fill truncated payloads to full length.
- [ ] Track `seq` per `(sysid, compid)` for loss statistics.
- [ ] Never accept `sysid = 0` or `compid = 0` as source.

**Signing**
- [ ] Per-link `link_id`; monotonically increasing 48-bit timestamp; persist.
- [ ] Verify SHA-256 (key ‖ header ‖ payload ‖ crc ‖ link_id ‖ timestamp).
- [ ] Reject stale (> 1 min) or replayed timestamps.
- [ ] Explicit policy for unsigned packets; conspicuous insecure indication.
- [ ] Never log keys; strip signatures from logs.

**Application**
- [ ] Emit `HEARTBEAT` at 1 Hz with correct `MAV_TYPE`/`MAV_AUTOPILOT`.
- [ ] Respond to `MAV_CMD_REQUEST_MESSAGE` for `AUTOPILOT_VERSION` (148) and
      `PROTOCOL_VERSION` (300); set `MAV_PROTOCOL_CAPABILITY_MAVLINK2`.
- [ ] Always answer commands with `COMMAND_ACK`; use `IN_PROGRESS` for long
      operations; honour `confirmation` retries idempotently.
- [ ] Prefer `*_INT` mission/command messages and explicit `MAV_FRAME`s.
- [ ] Route/forward per Part 6 without re-serializing.

## Appendix F — Source Pages

- Introduction: <https://mavlink.io/en/>
- Packet serialization: <https://mavlink.io/en/guide/serialization.html>
- Message signing: <https://mavlink.io/en/guide/message_signing.html>
- MAVLink 2: <https://mavlink.io/en/guide/mavlink_2.html>
- Version handshake: <https://mavlink.io/en/guide/mavlink_version.html>
- Routing: <https://mavlink.io/en/guide/routing.html>
- XML schema: <https://mavlink.io/en/guide/xml_schema.html>
- CRC-32: <https://mavlink.io/en/guide/crc.html>
- Microservices index: <https://mavlink.io/en/services/>
  - Heartbeat: <https://mavlink.io/en/services/heartbeat.html>
  - Command: <https://mavlink.io/en/services/command.html>
  - Mission: <https://mavlink.io/en/services/mission.html>
  - Parameter: <https://mavlink.io/en/services/parameter.html>
  - Manual control: <https://mavlink.io/en/services/manual_control.html>
  - FTP: <https://mavlink.io/en/services/ftp.html>
  - Camera v2: <https://mavlink.io/en/services/camera.html>
  - Gimbal v2: <https://mavlink.io/en/services/gimbal_v2.html>
- Common message set: <https://mavlink.io/en/messages/common.html>
- Minimal message set: <https://mavlink.io/en/messages/minimal.html>
- Message definitions repo: <https://github.com/mavlink/mavlink/tree/master/message_definitions/v1.0>
