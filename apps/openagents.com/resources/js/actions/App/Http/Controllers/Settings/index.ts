import ProfileController from './ProfileController'
import IntegrationController from './IntegrationController'

const Settings = {
    ProfileController: Object.assign(ProfileController, ProfileController),
    IntegrationController: Object.assign(IntegrationController, IntegrationController),
}

export default Settings