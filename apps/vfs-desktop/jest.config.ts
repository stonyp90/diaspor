export default {
  displayName: 'vfs-desktop',
  preset: '../../jest.preset.js',
  testEnvironment: 'jsdom',
  transform: {
    '^.+\\.[tj]sx?$': [
      'ts-jest',
      {
        tsconfig: {
          jsx: 'react-jsx',
        },
      },
    ],
  },
  transformIgnorePatterns: [
    '/node_modules/(?!(react-joyride|@tauri-apps|@testing-library)/)',
  ],
  moduleFileExtensions: ['ts', 'tsx', 'js', 'jsx'],
  coverageDirectory: '../../coverage/apps/vfs-desktop',
  testMatch: ['**/*.spec.ts', '**/*.spec.tsx'], // Include both TS and TSX test files
  testPathIgnorePatterns: [
    '/node_modules/',
    '/dist/',
    'FinderPage.spec.tsx', // Complex component with many dependencies - needs extensive mocking
    'useDraggablePanel.spec.tsx', // React hooks test environment issue
    'DraggableSection.spec.tsx', // React hooks test environment issue - similar to useDraggablePanel
    'TransferPanel.spec.tsx', // Causes memory issues in CI - too many async operations
    'UploadProgress.spec.tsx', // Timeout issues with async polling tests in CI
    'ObjectStoragePanel.spec.tsx', // Memory intensive - causes OOM on CI
  ],
  setupFilesAfterEnv: ['<rootDir>/src/setupTests.ts'],
  moduleNameMapper: {
    '\\.css$': 'identity-obj-proxy',
    '\\.module\\.css$': 'identity-obj-proxy',
    '^react$': '<rootDir>/../../node_modules/react',
    '^react-dom$': '<rootDir>/../../node_modules/react-dom',
    '^@testing-library/user-event$':
      '<rootDir>/../../node_modules/@testing-library/user-event',
  },
  testEnvironmentOptions: {
    customExportConditions: [''],
  },
  // Optimize for CI memory usage
  maxWorkers: process.env.CI ? 2 : '50%',
  testTimeout: process.env.CI ? 30000 : 15000, // Longer timeout for CI
  workerIdleMemoryLimit: '512MB',
};
