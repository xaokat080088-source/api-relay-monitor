import { ProviderType } from '../../types'
import { Provider } from './base'
import { MockProvider } from './mock'
import { XiaomaProvider } from './xiaoma'
import { JizhiProvider } from './jizhi'
import { JizhiNewProvider } from './jizhi_new'
import { XllmProvider } from './xllm'

export function getProvider(providerType: ProviderType): Provider {
  switch (providerType) {
    case 'xiaoma':
      return XiaomaProvider
    case 'jizhi':
      return JizhiProvider
    case 'jizhi_new':
      return JizhiNewProvider
    case 'xllm':
      return XllmProvider
    case 'mock':
    default:
      return MockProvider
  }
}

export { MockProvider, XiaomaProvider, JizhiProvider, JizhiNewProvider, XllmProvider }
export type { Provider }
