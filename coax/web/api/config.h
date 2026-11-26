// Coax Web centralized typed configuration (Issue #14076).
#pragma once
#include <stdbool.h>
#include <stdio.h>

typedef struct {
  int env;            // 0=local, 1=test, 2=staging, 3=production
  int mail_driver;    // 0=log, 1=capture, 2=resend
  int queue_driver;   // 0=sync, 1=postgres
  int storage_driver; // 0=local, 1=gcs
  int log_format;     // 0=text, 1=json
  int workers;
  bool has_db;
  bool seal_ok;
  bool pepper_ok;
  bool local_serve;
  bool has_resend_key;
  const char *origin;
} api_config;

void api_config_load(api_config *cfg);
int api_config_check(const api_config *cfg, char *err, size_t errcap);
void api_config_print_schema_json(FILE *out);
