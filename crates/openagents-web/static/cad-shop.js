// Browser events for the Rust CAD shop and checked server previews.
(function(){
  "use strict";
  document.querySelectorAll('[data-shop-form]').forEach(function(form){
    form.querySelectorAll('input[type=range]').forEach(function(input){input.addEventListener('input',function(){var text=input.closest('.oa-slider')?.querySelector('.oa-slider__value-text');if(text)text.textContent=input.value;});});
    form.addEventListener('submit',async function(event){
      var values=Array.from(form.querySelectorAll('input[type=range]')).map(function(input){return Number(input.value);});
      form.querySelector('[name=values]').value=JSON.stringify(values);
      if(!event.submitter || event.submitter.value!=='preview')return;
      event.preventDefault();
      var status=form.querySelector('[data-shop-status]'),buttons=Array.from(form.querySelectorAll('button[type=submit]'));
      buttons.forEach(function(b){b.disabled=true;});status.textContent='Updating the preview…';
      try{
        var data=new FormData(form);data.set('intent','preview');
        var response=await fetch(form.action,{method:'POST',body:new URLSearchParams(data)});
        if(!response.ok){var message=response.status===404?'This listing is no longer available.':'The preview could not be updated. Try again.';if((response.headers.get('content-type')||'').startsWith('text/plain')){var text=await response.text();if(text.length<512&&text.trim())message=text;}throw new Error(message);}
        var view=await response.json(),viewer=form.closest('[data-cad-shop]').querySelector('[data-cad-model]');
        if(!viewer.__cadSetModel)throw new Error('The 3D view is still loading. Try again.');
        viewer.__cadSetModel(view);status.textContent='Preview updated.';
      }catch(error){status.textContent=error.message||'The preview could not be updated. Try again.';}
      finally{buttons.forEach(function(b){b.disabled=false;});}
    });
  });
  document.querySelectorAll('[data-shop-copy]').forEach(function(button){button.addEventListener('click',async function(){
    var invoice=document.querySelector('[data-shop-invoice]'),status=document.querySelector('[data-shop-status]');
    try{await navigator.clipboard.writeText(invoice.value);status.textContent='Invoice copied.';}catch(_){invoice.select();status.textContent='Select and copy this invoice.';}
  });});
})();
