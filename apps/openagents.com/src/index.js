import { createPsionicApp } from '@openagentsinc/psionic';
import { home } from './routes/home';
import { agents } from './routes/agents';
import { docs } from './routes/docs';
import { about } from './routes/about';
const app = createPsionicApp({
    name: 'OpenAgents',
    port: 3003
});
// Define routes
app.route('/', home);
app.route('/agents', agents);
app.route('/docs', docs);
app.route('/about', about);
// Start the server
app.start();
//# sourceMappingURL=index.js.map