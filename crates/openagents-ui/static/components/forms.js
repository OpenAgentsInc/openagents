// Form control behavior for openagents-ui (UI-03).
//
// Registered with Alpine.data for the Alpine.js CSP build: markup only names a
// component (`x-data="oaTagInput"`), never an inline expression. Load this file
// before Alpine so the `alpine:init` listener is in place. Every control works
// without it: tag inputs are comma-separated text fields, sliders show their
// initial value, and textareas keep native sizing.
(function () {
  "use strict";

  function register(Alpine) {
    // SubmitWhenValid: the form's submit buttons stay disabled until every
    // field is valid (required, pattern). Without script the browser still
    // refuses an invalid form and the stylesheet shows the button disabled.
    Alpine.data("oaSubmitWhenValid", function () {
      return {
        init: function () {
          var form = this.$el;
          var sync = function () {
            var ok = form.checkValidity();
            form.querySelectorAll("button[type=submit]:not([formnovalidate])").forEach(function (button) {
              button.disabled = !ok;
              button.toggleAttribute("data-disabled", !ok);
            });
          };
          form.addEventListener("input", sync);
          form.addEventListener("change", sync);
          sync();
        }
      };
    });

    // TagInput: turns a comma-separated text field into removable tags. The
    // submitted value stays one field with the same name, joined by the
    // delimiter, so the server parses it the same way with or without JS.
    Alpine.data("oaTagInput", function () {
      return {
        tags: [],
        init: function () {
          var root = this.$el;
          var input = root.querySelector(".oa-tag-input__control");
          if (!input) return;
          var delimiter = root.getAttribute("data-delimiter") || ",";
          var max = parseInt(root.getAttribute("data-max") || "0", 10) || 0;
          var self = this;

          var hidden = document.createElement("input");
          hidden.type = "hidden";
          hidden.name = input.name;
          if (input.hasAttribute("form")) hidden.setAttribute("form", input.getAttribute("form"));
          input.removeAttribute("name");
          var required = input.required;
          root.appendChild(hidden);

          function parse(text) {
            return text
              .split(delimiter)
              .map(function (part) {
                return part.trim();
              })
              .filter(function (part) {
                return part.length > 0;
              });
          }

          function sync() {
            hidden.value = self.tags.join(delimiter);
            input.required = required && self.tags.length === 0;
            if (max > 0) input.readOnly = self.tags.length >= max;
          }

          function render() {
            root.querySelectorAll(".oa-tag-input__tag").forEach(function (node) {
              node.remove();
            });
            self.tags.forEach(function (tag, index) {
              var chip = document.createElement("span");
              chip.className = "oa-tag-input__tag";
              var value = document.createElement("span");
              value.className = "oa-tag-input__tag-value";
              value.textContent = tag;
              var remove = document.createElement("button");
              remove.type = "button";
              remove.className = "oa-tag-input__tag-remove";
              remove.setAttribute("aria-label", "Remove " + tag);
              remove.textContent = "×";
              remove.addEventListener("click", function (event) {
                event.stopPropagation();
                self.tags.splice(index, 1);
                render();
                input.focus();
              });
              chip.appendChild(value);
              chip.appendChild(remove);
              root.insertBefore(chip, input);
            });
            sync();
          }

          function add(text) {
            parse(text).forEach(function (tag) {
              var existing = self.tags.indexOf(tag);
              if (existing !== -1) {
                var chip = root.querySelectorAll(".oa-tag-input__tag")[existing];
                if (chip) {
                  chip.removeAttribute("data-duplicate");
                  void chip.offsetWidth;
                  chip.setAttribute("data-duplicate", "");
                }
                return;
              }
              if (max > 0 && self.tags.length >= max) return;
              self.tags.push(tag);
            });
            render();
          }

          this.tags = parse(input.value);
          input.value = "";
          render();

          input.addEventListener("keydown", function (event) {
            if (event.key === "Enter" || event.key === delimiter) {
              if (input.value.trim().length === 0 && event.key === "Enter") return;
              event.preventDefault();
              add(input.value);
              input.value = "";
            } else if (event.key === "Backspace" && input.value.length === 0 && self.tags.length > 0) {
              self.tags.pop();
              render();
            }
          });
          input.addEventListener("paste", function (event) {
            var text = (event.clipboardData || window.clipboardData).getData("text");
            if (text.indexOf(delimiter) === -1 && text.indexOf("\n") === -1) return;
            event.preventDefault();
            add(text.split("\n").join(delimiter));
          });
          input.addEventListener("blur", function () {
            if (input.value.trim().length > 0) {
              add(input.value);
              input.value = "";
            }
          });
          root.addEventListener("click", function (event) {
            if (event.target === root) input.focus();
          });
        },
      };
    });

    // Slider: keeps the filled track and the visible value in step with a
    // native range input.
    Alpine.data("oaSlider", function () {
      return {
        init: function () {
          var root = this.$el;
          var input = root.querySelector(".oa-slider__input");
          if (!input) return;
          var text = root.querySelector(".oa-slider__value-text");
          function update() {
            var min = parseFloat(input.min || "0");
            var max = parseFloat(input.max || "100");
            var value = parseFloat(input.value);
            var fill = max > min ? ((value - min) / (max - min)) * 100 : 0;
            input.style.setProperty("--oa-slider-fill", fill + "%");
            if (text) text.textContent = input.value;
          }
          input.addEventListener("input", update);
          update();
        },
      };
    });

    // Textarea auto-grow for browsers without `field-sizing: content`.
    Alpine.data("oaTextareaAutogrow", function () {
      return {
        init: function () {
          if (window.CSS && CSS.supports && CSS.supports("field-sizing", "content")) return;
          var area = this.$el.querySelector(".oa-textarea__control");
          if (!area) return;
          function resize() {
            area.style.height = "auto";
            area.style.height = area.scrollHeight + "px";
          }
          area.addEventListener("input", resize);
          resize();
        },
      };
    });
  }

  if (window.Alpine && typeof window.Alpine.data === "function") {
    register(window.Alpine);
  } else {
    document.addEventListener("alpine:init", function () {
      register(window.Alpine);
    });
  }
})();
