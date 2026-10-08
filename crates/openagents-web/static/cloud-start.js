try {
  const { default: init, start } = await import('/cloud/assets/coder_cloud_web.js');
  await init();
  start();
  if (document.getElementById('cloud-workbench-config')) {
    const terminal = await import('/cloud/assets/coder_browser_web.js');
    await terminal.default();
    await terminal.start();
  }
} catch {
  document.dispatchEvent(new Event('openagents-cloud-retired'));
  for (const input of document.querySelectorAll('.cloud input[type=password]')) {
    input.value = '';
    input.defaultValue = '';
    input.disabled = true;
  }
  const content = document.getElementById('cloud-private');
  if (content) {
    for (const input of content.querySelectorAll('input, textarea')) {
      input.value = '';
      input.defaultValue = '';
    }
    content.remove();
  } else {
    for (const form of document.querySelectorAll('.cloud form')) {
      for (const control of form.querySelectorAll('input, textarea, button, select')) {
        control.disabled = true;
      }
      const message = document.createElement('p');
      message.textContent = 'The workspace could not start. Reopen it to try again.';
      const link = document.createElement('a');
      link.href = '/cloud/app';
      link.textContent = 'Open workspace';
      form.replaceWith(message, link);
    }
  }
  const resume = document.getElementById('cloud-resume');
  if (resume) resume.hidden = false;
  document.title = 'Workspace · OpenAgents';
}
