// Coax Web centralized typed configuration loader & validator (Issue #14076).
#include "config.h"
#include "config-policy.generated.h"

#include <stdlib.h>
#include <string.h>

static coax_string cfg_cs(const char *s) {
  return (coax_string){s ? s : "", s ? (int64_t)strlen(s) : 0};
}

static const char *cfg_env(const char *name) {
  const char *v = getenv(name);
  return (v && *v) ? v : "";
}

void api_config_load(api_config *cfg) {
  memset(cfg, 0, sizeof *cfg);
  cfg->env = (int)cfg_parse_env(cfg_cs(cfg_env("OA_ENV")));
  cfg->mail_driver = (int)cfg_parse_mail(cfg_cs(cfg_env("OA_MAIL_DRIVER")));
  cfg->queue_driver = (int)cfg_parse_queue(cfg_cs(cfg_env("OA_QUEUE_DRIVER")));
  cfg->storage_driver = (int)cfg_parse_storage(cfg_cs(cfg_env("OA_STORAGE_DRIVER")));
  cfg->log_format = (int)cfg_parse_log_format(cfg_cs(cfg_env("OA_LOG_FORMAT")));
  const char *wk = cfg_env("OA_API_WORKERS");
  cfg->workers = *wk ? atoi(wk) : 4;
  if (cfg->workers < 1) cfg->workers = 1;
  if (cfg->workers > 32) cfg->workers = 32;
  cfg->has_db = *cfg_env("OA_API_DATABASE") != 0;
  cfg->seal_ok = cfg_valid_key_ring(cfg_cs(cfg_env("OA_AUTH_SEAL_KEY")));
  cfg->pepper_ok = cfg_valid_key_ring(cfg_cs(cfg_env("OA_AUTH_LOOKUP_PEPPER")));
#ifdef OA_LOCAL_SERVE
  cfg->local_serve = true;
#else
  cfg->local_serve = strcmp(cfg_env("OA_LOCAL_SERVE"), "1") == 0;
#endif
  cfg->has_resend_key = *cfg_env("OA_RESEND_API_KEY") != 0;
  const char *orig = cfg_env("OA_API_ORIGIN");
  cfg->origin = *orig ? orig : "https://openagents.com";
}

int api_config_check(const api_config *cfg, char *err, size_t errcap) {
  int code = (int)cfg_validate_boot(
      cfg->env, cfg->mail_driver, cfg->queue_driver, cfg->storage_driver,
      cfg->log_format, cfg->has_db, cfg->seal_ok, cfg->pepper_ok,
      cfg->local_serve, cfg->has_resend_key);
  if (err && errcap > 0) {
    coax_string msg = cfg_error_text(code);
    size_t n = (size_t)msg.count;
    if (n >= errcap) n = errcap - 1;
    if (n > 0) memcpy(err, msg.data, n);
    err[n] = 0;
  }
  return code;
}

void api_config_print_schema_json(FILE *out) {
  fputs(
      "{\n"
      "  \"format\": \"coax-web-config\",\n"
      "  \"version\": 1,\n"
      "  \"variables\": [\n"
      "    {\"name\":\"OA_ENV\",\"type\":\"enum\",\"values\":[\"local\",\"test\",\"staging\",\"production\"],\"default\":\"local\",\"secret\":false},\n"
      "    {\"name\":\"OA_API_DATABASE\",\"type\":\"string\",\"required\":true,\"secret\":true},\n"
      "    {\"name\":\"OA_API_ORIGIN\",\"type\":\"url\",\"default\":\"https://openagents.com\",\"secret\":false},\n"
      "    {\"name\":\"OA_API_SITE_ORIGINS\",\"type\":\"csv_origins\",\"default\":\"\",\"secret\":false},\n"
      "    {\"name\":\"OA_API_WORKERS\",\"type\":\"int\",\"min\":1,\"max\":32,\"default\":4,\"secret\":false},\n"
      "    {\"name\":\"OA_MAIL_DRIVER\",\"type\":\"enum\",\"values\":[\"log\",\"capture\",\"resend\"],\"default\":\"resend\",\"secret\":false},\n"
      "    {\"name\":\"OA_RESEND_API_KEY\",\"type\":\"string\",\"required_in\":[\"production\"],\"secret\":true},\n"
      "    {\"name\":\"OA_QUEUE_DRIVER\",\"type\":\"enum\",\"values\":[\"sync\",\"postgres\"],\"default\":\"postgres\",\"secret\":false},\n"
      "    {\"name\":\"OA_STORAGE_DRIVER\",\"type\":\"enum\",\"values\":[\"local\",\"gcs\"],\"default\":\"gcs\",\"secret\":false},\n"
      "    {\"name\":\"OA_LOG_FORMAT\",\"type\":\"enum\",\"values\":[\"text\",\"json\"],\"default\":\"text\",\"secret\":false},\n"
      "    {\"name\":\"OA_AUTH_SEAL_KEY\",\"type\":\"key_ring\",\"required_in\":[\"production\"],\"secret\":true},\n"
      "    {\"name\":\"OA_AUTH_LOOKUP_PEPPER\",\"type\":\"key_ring\",\"required_in\":[\"production\"],\"secret\":true},\n"
      "    {\"name\":\"OA_MAINTENANCE_BYPASS_SECRET\",\"type\":\"string\",\"default\":\"\",\"secret\":true}\n"
      "  ]\n"
      "}\n",
      out);
}
