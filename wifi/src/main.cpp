// RAMalyser Wi-Fi module: a Seeed XIAO ESP32-C6 on J8 of the base board.
//
// It serves the web interface (web/, built into web_assets.h) and passes
// commands to the tester over a serial link:
//
//   module D6 (GPIO16, TX) -> RP2350 GPIO1 (UART0 RX)
//   module D7 (GPIO17, RX) <- RP2350 GPIO0 (UART0 TX)      115200 8N1
//
// The tester answers each "json ..." line with one line of JSON.
//
//   GET  /api/state     {"busy":..,"result":<last answer or null>,"wifi":{..}}
//   POST /api/command   body: list | id | basic <n> | full <n>    -> 202, or 409 while busy
//   POST /api/wifi      form fields ssid, password                -> saves and restarts
//
// With no network saved, or when the saved one cannot be joined, the module
// opens its own network "RAMalyser-xxxx" and every address leads to the page.
//
// NOT COMPILED OR RUN YET.
#include <Arduino.h>
#include <DNSServer.h>
#include <ESPmDNS.h>
#include <Preferences.h>
#include <WebServer.h>
#include <WiFi.h>

#include "web_assets.h"

static const int LINK_TX = 16;                       // D6
static const int LINK_RX = 17;                       // D7
static const uint32_t LINK_BAUD = 115200;
static const uint32_t ANSWER_TIMEOUT_MS = 30UL * 60UL * 1000UL;   // a full test of a large module
static const uint32_t JOIN_TIMEOUT_MS = 20000;
static const size_t MAX_ANSWER = 4096;

static WebServer server(80);
static DNSServer dns;
static Preferences prefs;

static bool apMode = false;
static String apName;
static bool busy = false;
static uint32_t startedAt = 0;
static String answer;        // the last complete JSON line from the tester
static String incoming;      // the line being received

static String jsonString(const String &s) {
  String out = "\"";
  for (size_t i = 0; i < s.length(); i++) {
    char c = s[i];
    if (c == '"' || c == '\\') {
      out += '\\';
      out += c;
    } else if ((uint8_t)c < 0x20) {
      out += ' ';
    } else {
      out += c;
    }
  }
  return out + "\"";
}

static void pumpLink() {
  while (Serial1.available()) {
    char c = (char)Serial1.read();
    if (c == '\n') {
      if (busy && incoming.startsWith("{")) {
        answer = incoming;
        busy = false;
      }
      incoming = "";
    } else if (c != '\r' && incoming.length() < MAX_ANSWER) {
      incoming += c;
    }
  }
  if (busy && millis() - startedAt > ANSWER_TIMEOUT_MS) {
    answer = F("{\"ok\":false,\"error\":\"the tester did not answer\"}");
    busy = false;
  }
}

static void handleState() {
  String body = "{\"busy\":";
  body += busy ? "true" : "false";
  body += ",\"result\":";
  body += (busy || answer.length() == 0) ? String("null") : answer;
  body += ",\"wifi\":{\"mode\":";
  body += apMode ? "\"ap\"" : "\"sta\"";
  body += ",\"ssid\":";
  body += jsonString(apMode ? apName : WiFi.SSID());
  body += ",\"ip\":";
  body += jsonString((apMode ? WiFi.softAPIP() : WiFi.localIP()).toString());
  body += "}}";
  server.sendHeader("Cache-Control", "no-store");
  server.send(200, "application/json", body);
}

// Only the commands the page needs go to the tester. peek and poke stay on
// the USB console: poke can drive the memory bus.
static bool allowed(const String &cmd) {
  if (cmd.length() == 0 || cmd.length() > 16) return false;
  for (size_t i = 0; i < cmd.length(); i++) {
    char c = cmd[i];
    if (!(isAlphaNumeric(c) || c == ' ')) return false;
  }
  return cmd == "list" || cmd == "id" || cmd.startsWith("basic ") || cmd.startsWith("full ");
}

static void handleCommand() {
  String cmd = server.arg("plain");
  cmd.trim();
  if (!allowed(cmd)) {
    server.send(400, "application/json", F("{\"error\":\"unknown command\"}"));
    return;
  }
  if (busy) {
    server.send(409, "application/json", F("{\"error\":\"busy\"}"));
    return;
  }
  while (Serial1.available()) Serial1.read();        // drop anything stale
  incoming = "";
  answer = "";
  busy = true;
  startedAt = millis();
  Serial1.print("json ");
  Serial1.print(cmd);
  Serial1.print('\n');
  server.send(202, "application/json", F("{\"accepted\":true}"));
}

static void handleWifi() {
  String ssid = server.arg("ssid");
  String pass = server.arg("password");
  ssid.trim();
  if (ssid.length() == 0 || ssid.length() > 32 || pass.length() > 63) {
    server.send(400, "application/json", F("{\"error\":\"bad network name or password\"}"));
    return;
  }
  prefs.putString("ssid", ssid);
  prefs.putString("pass", pass);
  server.send(200, "application/json", F("{\"saved\":true}"));
  delay(500);
  ESP.restart();
}

static bool serveAsset(String path) {
  if (path == "/") path = "/index.html";
  for (size_t i = 0; i < WEB_ASSET_COUNT; i++) {
    if (path == WEB_ASSETS[i].path) {
      server.sendHeader("Content-Encoding", "gzip");
      server.sendHeader("Cache-Control", "no-cache");
      server.send_P(200, WEB_ASSETS[i].mime, (const char *)WEB_ASSETS[i].data, WEB_ASSETS[i].len);
      return true;
    }
  }
  return false;
}

static void handleOther() {
  if (serveAsset(server.uri())) return;
  if (apMode) {                                      // captive portal: any address leads here
    server.sendHeader("Location", String("http://") + WiFi.softAPIP().toString() + "/");
    server.send(302, "text/plain", "");
    return;
  }
  server.send(404, "text/plain", "not found");
}

static void startAccessPoint() {
  uint8_t mac[6];
  WiFi.macAddress(mac);
  char name[20];
  snprintf(name, sizeof name, "RAMalyser-%02x%02x", mac[4], mac[5]);
  apName = name;
  WiFi.mode(WIFI_AP);
  WiFi.softAP(name);
  dns.start(53, "*", WiFi.softAPIP());
  apMode = true;
}

void setup() {
  Serial.begin(115200);                              // the module's own USB port, for logs
  Serial1.begin(LINK_BAUD, SERIAL_8N1, LINK_RX, LINK_TX);
  prefs.begin("ramalyser", false);

  String ssid = prefs.getString("ssid", "");
  String pass = prefs.getString("pass", "");
  if (ssid.length() > 0) {
    WiFi.mode(WIFI_STA);
    WiFi.setHostname("ramalyser");
    WiFi.begin(ssid.c_str(), pass.c_str());
    uint32_t t0 = millis();
    while (WiFi.status() != WL_CONNECTED && millis() - t0 < JOIN_TIMEOUT_MS) delay(100);
  }
  if (WiFi.status() == WL_CONNECTED) {
    MDNS.begin("ramalyser");                         // http://ramalyser.local/
    MDNS.addService("http", "tcp", 80);
    Serial.print("joined, address ");
    Serial.println(WiFi.localIP());
  } else {
    startAccessPoint();
    Serial.print("own network ");
    Serial.println(apName);
  }

  server.on("/api/state", HTTP_GET, handleState);
  server.on("/api/command", HTTP_POST, handleCommand);
  server.on("/api/wifi", HTTP_POST, handleWifi);
  server.onNotFound(handleOther);
  server.begin();
}

void loop() {
  pumpLink();
  if (apMode) dns.processNextRequest();
  server.handleClient();
}
