import { describe, it, expect } from "bun:test";

/**
 * Unit tests for the binary protocol helpers used by the Worker.
 * These mirror the encoding/decoding logic in tunnel-registry.ts.
 * DO integration tests run against `wrangler dev`.
 */

const HEADER_SIZE = 9;
const FRAME_REQUEST = 0x03;
const FRAME_RESPONSE = 0x04;
const FRAME_PING = 0x05;
const FRAME_ERROR = 0x07;

function encodeRequest(
  requestId: number,
  method: number,
  url: string,
  headers: [string, string][],
  body: Uint8Array,
): ArrayBuffer {
  const urlBytes = new TextEncoder().encode(url);
  let payloadLen = 1 + 2 + urlBytes.length;
  payloadLen += 2;
  const headerBytesList: [Uint8Array, Uint8Array][] = [];
  for (const [name, value] of headers) {
    const nameBytes = new TextEncoder().encode(name);
    const valueBytes = new TextEncoder().encode(value);
    payloadLen += 2 + nameBytes.length + 2 + valueBytes.length;
    headerBytesList.push([nameBytes, valueBytes]);
  }
  payloadLen += 4 + body.length;

  const total = HEADER_SIZE + payloadLen;
  const buf = new ArrayBuffer(total);
  const arr = new Uint8Array(buf);
  const dv = new DataView(buf);

  arr[0] = FRAME_REQUEST;
  dv.setUint32(1, requestId, true);
  dv.setUint32(5, payloadLen, true);

  let offset = HEADER_SIZE;
  arr[offset++] = method;
  dv.setUint16(offset, urlBytes.length, true);
  offset += 2;
  arr.set(urlBytes, offset);
  offset += urlBytes.length;
  dv.setUint16(offset, headers.length, true);
  offset += 2;
  for (const [nameBytes, valueBytes] of headerBytesList) {
    dv.setUint16(offset, nameBytes.length, true);
    offset += 2;
    arr.set(nameBytes, offset);
    offset += nameBytes.length;
    dv.setUint16(offset, valueBytes.length, true);
    offset += 2;
    arr.set(valueBytes, offset);
    offset += valueBytes.length;
  }
  dv.setUint32(offset, body.length, true);
  offset += 4;
  arr.set(body, offset);
  return buf;
}

function parseFrameHeader(data: ArrayBuffer) {
  if (data.byteLength < HEADER_SIZE) return null;
  const buf = new Uint8Array(data);
  const dv = new DataView(data);
  const frameType = buf[0];
  const requestId = dv.getUint32(1, true);
  const payloadLen = dv.getUint32(5, true);
  if (data.byteLength < HEADER_SIZE + payloadLen) return null;
  const payload = buf.slice(HEADER_SIZE, HEADER_SIZE + payloadLen);
  return { frameType, requestId, payload };
}

function encodeResponse(
  requestId: number,
  status: number,
  headers: [string, string][],
  body: Uint8Array,
): ArrayBuffer {
  let payloadLen = 2 + 2;
  const headerBytesList: [Uint8Array, Uint8Array][] = [];
  for (const [name, value] of headers) {
    const nameBytes = new TextEncoder().encode(name);
    const valueBytes = new TextEncoder().encode(value);
    payloadLen += 2 + nameBytes.length + 2 + valueBytes.length;
    headerBytesList.push([nameBytes, valueBytes]);
  }
  payloadLen += 4 + body.length;

  const total = HEADER_SIZE + payloadLen;
  const buf = new ArrayBuffer(total);
  const arr = new Uint8Array(buf);
  const dv = new DataView(buf);

  arr[0] = FRAME_RESPONSE;
  dv.setUint32(1, requestId, true);
  dv.setUint32(5, payloadLen, true);

  let offset = HEADER_SIZE;
  dv.setUint16(offset, status, true);
  offset += 2;
  dv.setUint16(offset, headers.length, true);
  offset += 2;
  for (const [nameBytes, valueBytes] of headerBytesList) {
    dv.setUint16(offset, nameBytes.length, true);
    offset += 2;
    arr.set(nameBytes, offset);
    offset += nameBytes.length;
    dv.setUint16(offset, valueBytes.length, true);
    offset += 2;
    arr.set(valueBytes, offset);
    offset += valueBytes.length;
  }
  dv.setUint32(offset, body.length, true);
  offset += 4;
  arr.set(body, offset);
  return buf;
}

function encodePing(requestId: number): ArrayBuffer {
  const buf = new ArrayBuffer(HEADER_SIZE);
  const arr = new Uint8Array(buf);
  const dv = new DataView(buf);
  arr[0] = FRAME_PING;
  dv.setUint32(1, requestId, true);
  dv.setUint32(5, 0, true);
  return buf;
}

function encodeError(requestId: number, code: number, message: string): ArrayBuffer {
  const msgBytes = new TextEncoder().encode(message);
  const payloadLen = 2 + 2 + msgBytes.length;
  const total = HEADER_SIZE + payloadLen;
  const buf = new ArrayBuffer(total);
  const arr = new Uint8Array(buf);
  const dv = new DataView(buf);
  arr[0] = FRAME_ERROR;
  dv.setUint32(1, requestId, true);
  dv.setUint32(5, payloadLen, true);
  dv.setUint16(HEADER_SIZE, code, true);
  dv.setUint16(HEADER_SIZE + 2, msgBytes.length, true);
  arr.set(msgBytes, HEADER_SIZE + 4);
  return buf;
}

describe("Binary protocol encoding/decoding", () => {
  it("round-trips a REQUEST frame", () => {
    const body = new TextEncoder().encode('{"key":"value"}');
    const frame = encodeRequest(42, 1, "/api/test?q=1", [["content-type", "application/json"]], body);
    const parsed = parseFrameHeader(frame);
    expect(parsed).not.toBeNull();
    expect(parsed!.frameType).toBe(FRAME_REQUEST);
    expect(parsed!.requestId).toBe(42);
    expect(parsed!.payload[0]).toBe(1);
  });

  it("round-trips a RESPONSE frame", () => {
    const body = new TextEncoder().encode("hello");
    const frame = encodeResponse(99, 200, [["x-custom", "test"]], body);
    const parsed = parseFrameHeader(frame);
    expect(parsed).not.toBeNull();
    expect(parsed!.frameType).toBe(FRAME_RESPONSE);
    expect(parsed!.requestId).toBe(99);
    const dv = new DataView(parsed!.payload.buffer, parsed!.payload.byteOffset);
    expect(dv.getUint16(0, true)).toBe(200);
  });

  it("round-trips a PING frame", () => {
    const frame = encodePing(7);
    const parsed = parseFrameHeader(frame);
    expect(parsed).not.toBeNull();
    expect(parsed!.frameType).toBe(FRAME_PING);
    expect(parsed!.requestId).toBe(7);
    expect(parsed!.payload.length).toBe(0);
  });

  it("round-trips an ERROR frame", () => {
    const frame = encodeError(5, 502, "Bad Gateway");
    const parsed = parseFrameHeader(frame);
    expect(parsed).not.toBeNull();
    expect(parsed!.frameType).toBe(FRAME_ERROR);
    expect(parsed!.requestId).toBe(5);
    const dv = new DataView(parsed!.payload.buffer, parsed!.payload.byteOffset);
    expect(dv.getUint16(0, true)).toBe(502);
    const msgLen = dv.getUint16(2, true);
    expect(msgLen).toBe(11);
    const msg = new TextDecoder().decode(parsed!.payload.slice(4, 4 + msgLen));
    expect(msg).toBe("Bad Gateway");
  });

  it("returns null for too-short buffer", () => {
    const buf = new ArrayBuffer(4);
    expect(parseFrameHeader(buf)).toBeNull();
  });

  it("returns null for incomplete payload", () => {
    const buf = new ArrayBuffer(HEADER_SIZE);
    const dv = new DataView(buf);
    const arr = new Uint8Array(buf);
    arr[0] = FRAME_PING;
    dv.setUint32(1, 1, true);
    dv.setUint32(5, 100, true);
    expect(parseFrameHeader(buf)).toBeNull();
  });

  it("encodes empty body request", () => {
    const frame = encodeRequest(1, 0, "/", [], new Uint8Array(0));
    const parsed = parseFrameHeader(frame);
    expect(parsed).not.toBeNull();
    expect(parsed!.frameType).toBe(FRAME_REQUEST);
    expect(parsed!.requestId).toBe(1);
  });

  it("handles large request ids", () => {
    const frame = encodePing(0xFFFFFFFF);
    const parsed = parseFrameHeader(frame);
    expect(parsed).not.toBeNull();
    expect(parsed!.requestId).toBe(0xFFFFFFFF);
  });
});
