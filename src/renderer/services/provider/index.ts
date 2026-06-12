import { AppSettings } from '../../types'
import { Provider } from './base'
import { MockProvider } from './mock'
import { XiaomaProvider } from './xiaoma'

export function getProvider(settings: AppSettings): Provider {
  switch (settings.providerType) {
    case 'xiaoma':
      return XiaomaProvider
    case 'mock':
    default:
      return MockProvider
  }
}

export { MockProvider, XiaomaProvider }
export type { Provider }
