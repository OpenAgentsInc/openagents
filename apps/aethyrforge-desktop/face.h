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
void fgc_reset(void);
int64_t fgc_read(const uint8_t *p, int64_t n);
int64_t fgc_copy(int64_t field, uint8_t *p, int64_t cap);
bool fgc_send(bool busy, const uint8_t *p, int64_t n);
bool fgc_answer(bool busy, bool pending, const uint8_t *p, int64_t n);
int64_t fgc_catalog(int64_t i, bool label, uint8_t *out, int64_t cap);
void fgc_models_reset(void);
bool fgc_models(const uint8_t *p, int64_t n);
int64_t fgc_model(int64_t i, int64_t effort, uint8_t *out, int64_t cap);
float fgc_box(int64_t kind, float w, float h, int64_t component);
int64_t fgc_draw(float *out, int64_t cap, float w, float h, float d, bool busy);
