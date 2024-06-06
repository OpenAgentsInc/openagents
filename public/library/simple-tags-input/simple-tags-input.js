function simpleTagsInputCreateEvent(eventName, options) {
  /*
  * Creates event to communicate with the plugin,
  * in order to add/remove tag.
  */
  let eventOptions = {
      bubbles: true,
      cancelable: true,
      detail: options
  };
  const event = new CustomEvent(eventName, eventOptions);
  document.dispatchEvent(event);
}


function simpleTagsInputRemoveTag (e) {
  /*
  * Takes simpleTagsInput remove icon a.k.a. span element as argument
  * Fires event to communicate with the plugin,
  * in order to remove clicked tag.
  */
  let eventName = e.getAttribute("data-target");
  let value = e.getAttribute("data-value");
  let options = { "type": "removeTag", "tagValue": value, element: e };
  simpleTagsInputCreateEvent(eventName, options);
}

function simpleTagsInputAddTag (e) {
  /*
  * Takes simpleTagsInput list element as argument
  * Fires event to communicate with the plugin,
  * in order to add tag.
  */
  let eventName = e.getAttribute("data-target");
  let value = e.getAttribute("data-value");
  let options = { "type": "addTag", "tagValue": value, element: e };
  simpleTagsInputCreateEvent(eventName, options);
}


(function(options) {
  this.simpleTagsInput = function() {
      this.tags = [];
      this.searchItems = [];
      this.input = undefined;
      this.ul = undefined;
      this.bridgeID = Math.random().toString(29).substring(2) + new Date().getTime().toString() + Math.random().toString(29).substring(2);
      this.searchListEl = undefined;
      this.settings = (arguments[0] && typeof arguments[0] === 'object') ? arguments[0] : {};

      // initialize plugin
      initialize.call(this);
  }

  simpleTagsInput.prototype.getTags = function() {
      /* Returns tags list */
      return this.tags;
  }

  simpleTagsInput.prototype.addTag = function(value) {
      /* Add a new tag to the list */
      let tag = value.replace(/\s+/g, '');
      if( tag.length > 1 && !this.tags.includes(tag) ){
          tag.split(',').forEach(tag => {
              this.tags.push(tag);
              createTag.call(this);
          });
      }
  }

  simpleTagsInput.prototype.removeTag = function(tag) {
      /* Remove tag from the list */
      let index = this.tags.indexOf(tag);
      if ( index > -1 ) {
          this.tags = [...this.tags.slice(0, index), ...this.tags.slice(index + 1)];
          createTag.call(this);
      }
  }

  function initialize() {
      /* Init plugin basic functions/operations */
      let ok = setPluginParams.call(this);
      if (ok) {
          this.input.addEventListener("keyup", addTag.bind(this));
          createSearchListElElement.call(this);
          document.addEventListener(this.bridgeID, handleOutsidePluginTasks.bind(this));
          createTag.call(this);
      } else {
          throw new Error("simpleTagsInput: input or list element not found");
      }
  }

  function setPluginParams() {
      /*
      * set plugin params,
      * if required params were not set returns false,
      * else return true
      */
      try {
          let inputID = this.settings.inputEl ? this.settings.inputEl : undefined;
          let ulID = this.settings.listEl ? this.settings.listEl : undefined;
          this.searchItems = this.settings.autocompleteSearchList ? this.settings.autocompleteSearchList : [];
          let _input = document.getElementById(inputID);
          let _ul = document.getElementById(ulID);

          if ( _input.tagName == "INPUT" && _ul.tagName == "UL" ) {
              this.input = _input;
              this.ul = _ul;
              this.ul.classList.add("tagsList");
              return true
          } else {
              return false
          }
      } catch (e) {
          return false
      }
  }

  function createSearchListElElement() {
      // create search list `ul` element and set to `this.searchListEl`
      let random_id = Math.random().toString(30).substring(2);
      let el = `<ul id="${random_id}" class='simple-tags-input-search-list' style='display: none'></ul>`
      this.input.insertAdjacentHTML("afterend", el);
      this.searchListEl = document.getElementById(random_id);
  }

  function createTag () {
      /* Create tags from `this.tags` */
      this.ul.querySelectorAll("li").forEach(li => li.remove());
      this.tags.slice().reverse().forEach(tag => {
          let liTag = `<li>${tag} <span data-value='${tag}' data-target='${this.bridgeID}' onclick="simpleTagsInputRemoveTag(this)">X</span></li>`;
          this.ul.insertAdjacentHTML("afterbegin", liTag);
      });
  }

  function addTag (e) {
      /* Add tag / show auto complete search result -> from input element: `this.input` */
      let SPACE = " ";
      let ENTER = "Enter";
      let COMMA = ",";
      if(e.key == SPACE || e.key == ENTER || e.key == COMMA) {
          // isert new tag
          let tag = e.target.value.replace(/\s+/g, '');
          if( tag.length > 1 && !this.tags.includes(tag) ){
              this.tags.push(tag);
              createTag.call(this);
          }
          e.target.value = "";
      } else {
          // autocomplete search
          let q = e.target.value.replace(/\s+/g, ' ');
          if (q.length > 0) {
              autoCompleteSearch.call(this, e);
          } else {
              this.searchListEl.style.display = "none";
          }
      }
  }

  function autoCompleteSearch(e) {
      /* Handles auto complete search */
      let q = e.target.value;
      let results = this.searchItems.filter(item => item.toLowerCase().indexOf(q.toLowerCase()) != -1)
      let _html = "<p style='border-bottom: 1px solid lightgrey; margin-bottom: 0px; font-weight: bold; padding: 5px; font-style: italic '>Search Result:</p>";
      results.forEach(item => {
          _html += `<li data-value='${item}' data-target='${this.bridgeID}' onclick="simpleTagsInputAddTag(this)">${item}</li>`;
      });
      this.searchListEl.innerHTML = _html;

      if ( results.length == 0 ) {
          this.searchListEl.style.display = "none";
      } else {
          this.searchListEl.style.display = "block";
      }
  }

  function handleOutsidePluginTasks(e) {
      /*
      * Handles outside plugin tasks
      * Create/remove tag from outside of the plugin via event.
      */
      let eventType = e.detail.type;
      let tag = e.detail.tagValue;
      let element = e.detail.element;
      if ( eventType == "addTag" ) addTagFromOutside.call(this, tag);
      if ( eventType == "removeTag" ) removeTagFromOutside.call(this, element, tag);
  }

  function addTagFromOutside (tag) {
      /* Add tag from outside of the plugin via event */
      this.input.value = "";
      this.searchListEl.style.display = "none";
      if( tag.length > 1 && !this.tags.includes(tag) ){
          this.tags.push(tag);
          createTag.call(this);
      }
  }

  function removeTagFromOutside(element, tag){
      /* Remove tag from outside of the plugin via event */
      let index = this.tags.indexOf(tag);
      if ( index > -1 ) {
          this.tags = [...this.tags.slice(0, index), ...this.tags.slice(index + 1)];
          element.parentElement.remove();
      }
  }
}());
