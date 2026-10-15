// File and form glue; Rust checks, imports, and versions the model.
for(const form of document.querySelectorAll('[data-cad-import-form]')){
  const button=form.querySelector('button[type=submit]'),status=form.querySelector('[data-cad-import-status]');
  button.disabled=false;for(const name of ['aria-disabled','tabindex','data-disabled'])button.removeAttribute(name);
  form.addEventListener('submit',async event=>{
    event.preventDefault();const file=form.querySelector('[data-cad-import-file]').files[0];
    if(!file)return;if(file.size>2*1024*1024){status.textContent='CAD imports must be 2 MiB or smaller.';return;}
    button.disabled=true;status.textContent='Checking the model…';
    try{
      const query=new URLSearchParams({design:form.querySelector('[data-cad-import-design]').value,project:form.dataset.cadImportProject,part:form.querySelector('[data-cad-import-part]').value,feature:form.querySelector('[data-cad-import-feature]').value});
      const response=await fetch(`${form.dataset.cadImportForm}?${query}`,{method:'POST',credentials:'same-origin',headers:{'content-type':'application/octet-stream','x-csrf-token':form.dataset.cadImportCsrf},body:file});
      if(!response.ok)throw new Error(await response.text());const result=await response.json();location.assign(result.url);
    }catch(error){status.textContent=error.message;button.disabled=false;}
  });
}
