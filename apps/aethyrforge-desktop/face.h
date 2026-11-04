// Scalar Swift bridge to the Coax setup state. No borrowed Coax storage escapes.
#include <stdbool.h>
#include <stdint.h>
int64_t forge_url_byte(int64_t page, int64_t room, int64_t index);
void fgs_reset(void);
int64_t fgs_input_byte(int64_t index, int64_t value);
int64_t fgs_accept(int64_t kind, int64_t length);
bool fgs_can_enter(bool scene);
void fgs_choose(int64_t kind);
int64_t fgs_count(int64_t kind);
int64_t fgs_selected(void);
int64_t fgs_status(int64_t kind);
int64_t fgs_text_byte(int64_t field, int64_t index);
int64_t fgs_settings_byte(int64_t index);
