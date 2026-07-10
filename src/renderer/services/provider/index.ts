import { ProviderType } from '../../types'
import { Provider } from './base'
import { MockProvider } from './mock'
import { XiaomaProvider } from './xiaoma'
import { JizhiProvider } from './jizhi'

export function getProvider(providerType: ProviderType): Provider {
  switch (providerType) {
    case 'xiaoma':
      return XiaomaProvider
    case 'jizhi':
      return JizhiProvider
    case 'mock':
    default:
      return MockProvider
  }
}

export { MockProvider, XiaomaProvider, JizhiProvider }
export type { Provider }
