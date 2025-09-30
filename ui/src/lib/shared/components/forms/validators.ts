import type { Validator } from 'svelte-forms';
import pkg from 'ipaddr.js';

const { isValid, isValidCIDR, parse, parseCIDR } = pkg;

// IP Address validator
export const ipAddress = (): Validator => (value: string) => {
	if (!value) return { valid: true, name: 'Valid IP' }; // Allow empty if not required

	if (!isValid(value)) {
		return { name: 'Invalid IP', message: 'Invalid IP address format', valid: false };
	}

	return {
		valid: true,
		name: 'Valid IP'
	};
};

// CIDR validator
export const cidr = (): Validator => (value: string) => {
	if (!value) return { valid: true, name: 'Valid CIDR' }; // Allow empty if not required

	if (!isValidCIDR(value)) return { valid: false, name: `${cidr} is not valid CIDR notation` };

	return {
		valid: true,
		name: 'Valid CIDR'
	};
};

// IP in CIDR validator
export const ipAddressInCidr =
	(cidr: string): Validator =>
	(value: string) => {
		if (!isValidCIDR(cidr)) return { valid: false, name: `${cidr} is not valid CIDR notation` };
		if (!isValid(value))
			return { valid: true, name: `IP invalid, ipAddress validator will handle` };
		if (!parse(value).match(parseCIDR(cidr)))
			return { valid: false, name: `IP not in range of ${cidr}` };

		return {
			valid: true,
			name: 'IP in CIDR range'
		};
	};

// MAC address validator
export const mac = (): Validator => (value: string) => {
	if (!value) return { name: 'validMac', valid: true }; // Optional field

	const macRegex = /^([0-9A-Fa-f]{2}[:-]){5}([0-9A-Fa-f]{2})$/;
	if (!macRegex.test(value)) {
		return { name: 'Invalid MAC', message: 'Invalid MAC address format', valid: false };
	}

	return { name: 'Valid MAC', valid: true };
};

// Hostname validator
export const hostname = (): Validator => (value: string) => {
	if (!value) return { valid: true, name: 'hostname' }; // Allow empty if not required

	const hostnameRegex =
		/^([a-zA-Z0-9]([a-zA-Z0-9-]{0,61}[a-zA-Z0-9])?\.)*[a-zA-Z0-9]([a-zA-Z0-9-]{0,61}[a-zA-Z0-9])?$/;
	return {
		valid: hostnameRegex.test(value.trim()),
		name: 'Please enter a valid hostname',
		message: 'Please enter a valid hostname'
	};
};

// Max length validator with custom message
export const maxLength =
	(max: number): Validator =>
	(value: string) => {
		if (!value) return { valid: true, name: 'maxLength' };

		return {
			valid: value.length <= max,
			name: `Must be less than ${max} characters`,
			message: `Must be less than ${max} characters`
		};
	};

// Required field validator for non-editing contexts
export const requiredForNew =
	(isEditing: boolean): Validator =>
	(value: string) => {
		if (isEditing) return { valid: true, name: 'requiredForNew' };

		return {
			valid: Boolean(value && value.trim()),
			name: 'This field is required for new entries',
			message: 'This field is required for new entries'
		};
	};

// Port range validator
export const portRange = (): Validator => (value: number | string) => {
	if (!value && value !== 0) return { valid: true, name: 'portRange' };

	const port = typeof value === 'string' ? parseInt(value) : value;
	return {
		valid: Number.isInteger(port) && port >= 1 && port <= 65535,
		name: 'Port must be between 1 and 65535',
		message: 'Port must be between 1 and 65535'
	};
};
